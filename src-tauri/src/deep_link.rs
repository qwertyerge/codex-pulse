use anyhow::Result;

pub fn thread_url(thread_id: &str) -> Result<String> {
    let thread_id = uuid::Uuid::parse_str(thread_id)?;
    Ok(format!("codex://threads/{thread_id}"))
}

#[cfg(test)]
mod tests {
    use super::thread_url;

    #[test]
    fn accepts_only_uuid_thread_ids() {
        assert_eq!(
            thread_url("00000000-0000-4000-8000-000000000001").unwrap(),
            "codex://threads/00000000-0000-4000-8000-000000000001"
        );
        assert!(thread_url("$(open bad)").is_err());
        assert!(thread_url("../bad").is_err());
    }
}
