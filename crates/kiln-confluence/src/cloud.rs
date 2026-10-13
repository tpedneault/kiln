//! Confluence Cloud: the v2 REST API under `<url>/api/v2` (with v1's
//! `whoami`). v2's child and descendant listings carry no versions, so
//! each listed page is read once more for its version (**verify** whether
//! `descendants` returns more fields on live sites). Version authors are
//! account ids: v2 doesn't name people.

use std::sync::Mutex;

use serde_json::{json, Value};

use crate::client::{
    parse_property, query, Body, ClientError, Confluence, Created, Http, Identity,
};
use crate::model::{KilnProperty, RemotePage};

pub struct Cloud {
    pub http: Http,
    /// Space ids by key: v2 creates pages by space id.
    pub spaces: Mutex<Vec<(String, u64)>>,
}

fn id_of(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

fn page_from(v: &Value) -> Option<RemotePage> {
    Some(RemotePage {
        id: id_of(v.get("id")?)?,
        title: v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        parent_id: v.get("parentId").and_then(id_of),
        version: v
            .pointer("/version/number")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        version_author: v
            .pointer("/version/authorId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        version_when: v
            .pointer("/version/createdAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        property: None,
        body_hash: None,
        empty: false,
        leftover: false,
    })
}

impl Cloud {
    pub fn new(http: Http) -> Self {
        Cloud {
            http,
            spaces: Mutex::new(Vec::new()),
        }
    }

    fn space_id(&self, key: &str) -> Result<u64, ClientError> {
        if let Some(id) = self
            .spaces
            .lock()
            .ok()
            .and_then(|s| s.iter().find(|(k, _)| k == key).map(|(_, id)| *id))
        {
            return Ok(id);
        }
        let spaces = self
            .http
            .get(&format!("/api/v2/spaces?keys={}", query(key)))?
            .unwrap_or_default();
        let id = spaces
            .pointer("/results/0/id")
            .and_then(id_of)
            .ok_or_else(|| {
                ClientError::new(format!(
                    "Confluence space {key} doesn't exist or the token can't see it"
                ))
            })?;
        if let Ok(mut s) = self.spaces.lock() {
            s.push((key.to_owned(), id));
        }
        Ok(id)
    }

    /// Listed pages with their versions read.
    fn with_versions(&self, listed: Vec<Value>) -> Result<Vec<RemotePage>, ClientError> {
        let mut out = Vec::new();
        for v in &listed {
            let Some(id) = v.get("id").and_then(id_of) else {
                continue;
            };
            if let Some(mut page) = self.page(id)? {
                if page.parent_id.is_none() {
                    page.parent_id = v.get("parentId").and_then(id_of);
                }
                out.push(page);
            }
        }
        Ok(out)
    }
}

impl Confluence for Cloud {
    fn whoami(&self) -> Result<String, ClientError> {
        let v = self.http.get("/rest/api/user/current")?.unwrap_or_default();
        Ok(v.get("displayName")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned())
    }

    fn page(&self, id: u64) -> Result<Option<RemotePage>, ClientError> {
        Ok(self
            .http
            .get(&format!("/api/v2/pages/{id}"))?
            .as_ref()
            .and_then(page_from))
    }

    fn children(&self, id: u64) -> Result<Vec<RemotePage>, ClientError> {
        let listed = self
            .http
            .get_all(&format!("/api/v2/pages/{id}/children?limit=250"))?;
        self.with_versions(listed)
    }

    fn descendants(&self, id: u64) -> Result<Vec<RemotePage>, ClientError> {
        let listed = self
            .http
            .get_all(&format!("/api/v2/pages/{id}/descendants?limit=250"))?;
        self.with_versions(listed)
    }

    fn property(&self, id: u64) -> Result<Option<KilnProperty>, ClientError> {
        let v = self
            .http
            .get(&format!("/api/v2/pages/{id}/properties?key=kiln"))?
            .unwrap_or_default();
        Ok(v.pointer("/results/0/value").and_then(parse_property))
    }

    fn body(&self, id: u64) -> Result<String, ClientError> {
        let v = self
            .http
            .get(&format!("/api/v2/pages/{id}?body-format=storage"))?
            .unwrap_or_default();
        Ok(v.pointer("/body/storage/value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned())
    }

    fn find_title(&self, space: &str, title: &str) -> Result<Option<u64>, ClientError> {
        let space_id = self.space_id(space)?;
        let v = self
            .http
            .get(&format!(
                "/api/v2/pages?space-id={space_id}&title={}",
                query(title)
            ))?
            .unwrap_or_default();
        Ok(v.pointer("/results/0/id").and_then(id_of))
    }

    fn identity(&self) -> Result<Identity, ClientError> {
        let v = self.http.get("/rest/api/user/current")?.unwrap_or_default();
        let field = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        Ok(Identity {
            name: field("displayName"),
            key: field("accountId"),
        })
    }

    fn create_page(
        &self,
        space: &str,
        parent: u64,
        title: &str,
        body: &str,
    ) -> Result<Created, ClientError> {
        let request = json!({
            "spaceId": self.space_id(space)?.to_string(), "status": "current", "title": title,
            "parentId": parent.to_string(),
            "body": { "representation": "storage", "value": body },
        });
        let v = self
            .http
            .request("POST", "/api/v2/pages", Body::Json(request))?
            .unwrap_or_default();
        crate::dc::created(&v)
    }

    fn update_page(
        &self,
        id: u64,
        parent: Option<u64>,
        title: &str,
        body: &str,
        version: u64,
        message: &str,
    ) -> Result<u64, ClientError> {
        let mut request = json!({
            "id": id.to_string(), "status": "current", "title": title,
            "body": { "representation": "storage", "value": body },
            "version": { "number": version, "message": message },
        });
        // Moving through v2's update (**verify**; v1's move endpoint is the
        // fallback CONFLUENCE.md names).
        if let Some(parent) = parent {
            request["parentId"] = json!(parent.to_string());
        }
        let v = self
            .http
            .request("PUT", &format!("/api/v2/pages/{id}"), Body::Json(request))?
            .unwrap_or_default();
        Ok(v.pointer("/version/number")
            .and_then(Value::as_u64)
            .unwrap_or(version))
    }

    fn write_property(&self, id: u64, property: &KilnProperty) -> Result<(), ClientError> {
        let value = serde_json::to_value(property).map_err(|e| ClientError::new(e.to_string()))?;
        let current = self
            .http
            .get(&format!("/api/v2/pages/{id}/properties?key=kiln"))?
            .unwrap_or_default();
        let existing = current.pointer("/results/0/id").and_then(id_of);
        let version = current
            .pointer("/results/0/version/number")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        match existing {
            Some(pid) => {
                let request =
                    json!({ "key": "kiln", "value": value, "version": { "number": version + 1 } });
                self.http.request(
                    "PUT",
                    &format!("/api/v2/pages/{id}/properties/{pid}"),
                    Body::Json(request),
                )?;
            }
            None => {
                let request = json!({ "key": "kiln", "value": value });
                self.http.request(
                    "POST",
                    &format!("/api/v2/pages/{id}/properties"),
                    Body::Json(request),
                )?;
            }
        }
        Ok(())
    }

    fn labels(&self, id: u64) -> Result<Vec<String>, ClientError> {
        let all = self
            .http
            .get_all(&format!("/api/v2/pages/{id}/labels?limit=200"))?;
        Ok(all
            .iter()
            .filter_map(|l| l.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect())
    }

    fn add_labels(&self, id: u64, labels: &[String]) -> Result<(), ClientError> {
        // v2 has no label writes (CONFCLOUD-76866): v1.
        let request: Vec<Value> = labels
            .iter()
            .map(|l| json!({ "prefix": "global", "name": l }))
            .collect();
        self.http.request(
            "POST",
            &format!("/rest/api/content/{id}/label"),
            Body::Json(Value::Array(request)),
        )?;
        Ok(())
    }

    fn attachments(&self, id: u64) -> Result<Vec<(String, String)>, ClientError> {
        let all = self
            .http
            .get_all(&format!("/api/v2/pages/{id}/attachments?limit=250"))?;
        Ok(all
            .iter()
            .filter_map(|a| {
                let id = match a.get("id")? {
                    Value::String(s) => s.clone(),
                    v => v.to_string(),
                };
                Some((id, a.get("title")?.as_str()?.to_owned()))
            })
            .collect())
    }

    fn upload(&self, id: u64, name: &str, bytes: &[u8], mime: &str) -> Result<(), ClientError> {
        // v2 attachments are read-only: v1.
        let body = Body::Multipart {
            name: name.to_owned(),
            bytes: bytes.to_vec(),
            mime: mime.to_owned(),
        };
        self.http.request(
            "POST",
            &format!("/rest/api/content/{id}/child/attachment"),
            body,
        )?;
        Ok(())
    }

    fn delete_attachment(&self, attachment_id: &str) -> Result<(), ClientError> {
        self.http.request(
            "DELETE",
            &format!("/api/v2/attachments/{attachment_id}"),
            Body::None,
        )?;
        Ok(())
    }

    fn restrict_edits(&self, id: u64, who: &Identity) -> Result<(), ClientError> {
        let request = json!([{ "operation": "update", "restrictions": { "user": [{ "type": "known", "accountId": who.key }] } }]);
        self.http.request(
            "PUT",
            &format!("/rest/api/content/{id}/restriction"),
            Body::Json(request),
        )?;
        Ok(())
    }

    fn trash(&self, id: u64) -> Result<(), ClientError> {
        self.http
            .request("DELETE", &format!("/api/v2/pages/{id}"), Body::None)?;
        Ok(())
    }
}
