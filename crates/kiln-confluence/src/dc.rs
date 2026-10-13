//! Confluence Data Center: the v1 REST API under `<url>/rest/api`.

use serde_json::Value;

use crate::client::{parse_property, query, ClientError, Confluence, Http};
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
}
