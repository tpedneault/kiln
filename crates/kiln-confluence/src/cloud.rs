//! Confluence Cloud: the v2 REST API under `<url>/api/v2` (with v1's
//! `whoami`). v2's child and descendant listings carry no versions, so
//! each listed page is read once more for its version (**verify** whether
//! `descendants` returns more fields on live sites). Version authors are
//! account ids: v2 doesn't name people.

use serde_json::Value;

use crate::client::{parse_property, query, ClientError, Confluence, Http};
use crate::model::{KilnProperty, RemotePage};

pub struct Cloud {
    pub http: Http,
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
    })
}

impl Cloud {
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
        let spaces = self
            .http
            .get(&format!("/api/v2/spaces?keys={}", query(space)))?
            .unwrap_or_default();
        let Some(space_id) = spaces.pointer("/results/0/id").and_then(id_of) else {
            return Err(ClientError {
                message: format!(
                    "Confluence space {space} doesn't exist or the token can't see it"
                ),
            });
        };
        let v = self
            .http
            .get(&format!(
                "/api/v2/pages?space-id={space_id}&title={}",
                query(title)
            ))?
            .unwrap_or_default();
        Ok(v.pointer("/results/0/id").and_then(id_of))
    }
}
