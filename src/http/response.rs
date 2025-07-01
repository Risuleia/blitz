use std::collections::HashMap;

#[derive(Debug)]
pub struct HttpResponse {
    pub status_code: u16,
    pub reason_phrase: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>
}

impl HttpResponse {
    pub fn to_string(&self) -> String {
        let mut res = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.reason_phrase);

        for (key, value) in &self.headers {
            res.push_str(&format!("{}: {}\r\n", key, value));
        }

        res.push_str("\r\n");

        if let Some(body) = &self.body {
            res.push_str(body);
        }

        res
    }
}