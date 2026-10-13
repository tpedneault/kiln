//! Confluence Data Center: the v1 REST API under `<url>/rest/api`.

use serde_json::{json, Value};

use crate::client::{
    parse_property, query, Body, ClientError, Confluence, Created, Http, Identity,
};
use crate::model::{KilnProperty, RemotePage};

pub struct DataCenter {
    pub http: Http,
}

fn page_from(v: &Value) -> Option<RemotePage> {
    let id = v.get("id")?.as_str()?.parse().ok()?;
    let parent_id = v
        .get("ancestors")
        .and_then(Value::as_array)
        .and_then(|a| a.last())
        .and_then(|a| a.get("id"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok());
    Some(RemotePage {
        id,
        title: v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        parent_id,
        version: v
            .pointer("/version/number")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        version_author: v
            .pointer("/version/by/displayName")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        version_when: v
            .pointer("/version/when")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        property: None,
        body_hash: None,
        empty: false,
        leftover: false,
    })
}

impl Confluence for DataCenter {
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
            .get(&format!(
                "/rest/api/content/{id}?expand=version,ancestors,space"
            ))?
            .as_ref()
            .and_then(page_from))
    }

    fn children(&self, id: u64) -> Result<Vec<RemotePage>, ClientError> {
        let all = self.http.get_all(&format!(
            "/rest/api/content/{id}/child/page?expand=version,ancestors&limit=100"
        ))?;
        Ok(all.iter().filter_map(page_from).collect())
    }

    fn descendants(&self, id: u64) -> Result<Vec<RemotePage>, ClientError> {
        let all = self.http.get_all(&format!(
            "/rest/api/content/{id}/descendant/page?expand=version,ancestors&limit=100"
        ))?;
        Ok(all.iter().filter_map(page_from).collect())
    }

    fn property(&self, id: u64) -> Result<Option<KilnProperty>, ClientError> {
        Ok(self
            .http
            .get(&format!("/rest/api/content/{id}/property/kiln"))?
            .and_then(|v| v.get("value").and_then(parse_property)))
    }

    fn body(&self, id: u64) -> Result<String, ClientError> {
        let v = self
            .http
            .get(&format!(
                "/rest/api/content/{id}?expand=body.storage,version"
            ))?
            .unwrap_or_default();
        Ok(v.pointer("/body/storage/value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned())
    }

    fn find_title(&self, space: &str, title: &str) -> Result<Option<u64>, ClientError> {
        let v = self
            .http
            .get(&format!(
                "/rest/api/content?spaceKey={}&title={}&type=page",
                query(space),
                query(title)
            ))?
            .unwrap_or_default();
        Ok(v.pointer("/results/0/id")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok()))
    }

    fn identity(&self) -> Result<Identity, ClientError> {
        let v = self.http.get("/rest/api/user/current")?.unwrap_or_default();
        let field = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        Ok(Identity {
            name: field("displayName"),
            key: field("username"),
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
            "type": "page", "title": title, "space": { "key": space },
            "ancestors": [{ "id": parent.to_string() }],
            "body": { "storage": { "value": body, "representation": "storage" } },
        });
        let v = self
            .http
            .request("POST", "/rest/api/content", Body::Json(request))?
            .unwrap_or_default();
        created(&v)
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
            "id": id.to_string(), "type": "page", "title": title,
            "version": { "number": version, "message": message },
            "body": { "storage": { "value": body, "representation": "storage" } },
        });
        if let Some(parent) = parent {
            request["ancestors"] = json!([{ "id": parent.to_string() }]);
        }
        let v = self
            .http
            .request(
                "PUT",
                &format!("/rest/api/content/{id}"),
                Body::Json(request),
            )?
            .unwrap_or_default();
        Ok(v.pointer("/version/number")
            .and_then(Value::as_u64)
            .unwrap_or(version))
    }

    fn write_property(&self, id: u64, property: &KilnProperty) -> Result<(), ClientError> {
        let value = serde_json::to_value(property).map_err(|e| ClientError::new(e.to_string()))?;
        let current = self
            .http
            .get(&format!("/rest/api/content/{id}/property/kiln"))?;
        match current.and_then(|c| c.pointer("/version/number").and_then(Value::as_u64)) {
            Some(n) => {
                let request =
                    json!({ "key": "kiln", "value": value, "version": { "number": n + 1 } });
                self.http.request(
                    "PUT",
                    &format!("/rest/api/content/{id}/property/kiln"),
                    Body::Json(request),
                )?;
            }
            None => {
                let request = json!({ "key": "kiln", "value": value });
                self.http.request(
                    "POST",
                    &format!("/rest/api/content/{id}/property"),
                    Body::Json(request),
                )?;
            }
        }
        Ok(())
    }

    fn labels(&self, id: u64) -> Result<Vec<String>, ClientError> {
        let all = self
            .http
            .get_all(&format!("/rest/api/content/{id}/label?limit=200"))?;
        Ok(all
            .iter()
            .filter_map(|l| l.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect())
    }

    fn add_labels(&self, id: u64, labels: &[String]) -> Result<(), ClientError> {
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
        let all = self.http.get_all(&format!(
            "/rest/api/content/{id}/child/attachment?limit=200"
        ))?;
        Ok(all
            .iter()
            .filter_map(|a| {
                Some((
                    a.get("id")?.as_str()?.to_owned(),
                    a.get("title")?.as_str()?.to_owned(),
                ))
            })
            .collect())
    }

    fn upload(&self, id: u64, name: &str, bytes: &[u8], mime: &str) -> Result<(), ClientError> {
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
            &format!("/rest/api/content/{attachment_id}"),
            Body::None,
        )?;
        Ok(())
    }

    fn restrict_edits(&self, id: u64, who: &Identity) -> Result<(), ClientError> {
        let request = json!([{ "operation": "update", "restrictions": { "user": [{ "type": "known", "username": who.key }] } }]);
        self.http.request(
            "PUT",
            &format!("/rest/api/content/{id}/restriction"),
            Body::Json(request),
        )?;
        Ok(())
    }

    fn trash(&self, id: u64) -> Result<(), ClientError> {
        self.http
            .request("DELETE", &format!("/rest/api/content/{id}"), Body::None)?;
        Ok(())
    }
}

/// The id and version of a page a create returned.
pub fn created(v: &Value) -> Result<Created, ClientError> {
    let id = match v.get("id") {
        Some(Value::String(s)) => s.parse().ok(),
        Some(Value::Number(n)) => n.as_u64(),
        _ => None,
    };
    let id =
        id.ok_or_else(|| ClientError::new("Confluence created a page but didn't say its id"))?;
    Ok(Created {
        id,
        version: v
            .pointer("/version/number")
            .and_then(Value::as_u64)
            .unwrap_or(1),
    })
}
