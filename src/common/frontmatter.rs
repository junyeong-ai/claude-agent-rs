use serde::de::DeserializeOwned;

pub struct ParsedDocument<F> {
    pub frontmatter: F,
    pub body: String,
}

pub fn parse_frontmatter<F: DeserializeOwned>(content: &str) -> crate::Result<ParsedDocument<F>> {
    if !content.starts_with("---") {
        return Err(crate::Error::Config(
            "Document must have YAML frontmatter (starting with ---)".to_string(),
        ));
    }

    let after_first = &content[3..];
    let end_pos = after_first.find("---").ok_or_else(|| {
        crate::Error::Config("Frontmatter not properly terminated with ---".to_string())
    })?;

    let frontmatter_str = after_first[..end_pos].trim();
    let body = after_first[end_pos + 3..].trim().to_string();

    let frontmatter: F = serde_yaml_ng::from_str(frontmatter_str)
        .map_err(|e| crate::Error::Config(format!("Failed to parse frontmatter: {}", e)))?;

    Ok(ParsedDocument { frontmatter, body })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestFrontmatter {
        name: String,
        #[serde(default)]
        description: String,
    }

    #[test]
    fn test_parse_valid() {
        let content = r#"---
name: test
description: A test
---

Body content here."#;

        let doc = parse_frontmatter::<TestFrontmatter>(content).unwrap();
        assert_eq!(doc.frontmatter.name, "test");
        assert_eq!(doc.frontmatter.description, "A test");
        assert_eq!(doc.body, "Body content here.");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "Just content without frontmatter";
        let result = parse_frontmatter::<TestFrontmatter>(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unterminated() {
        let content = "---\nname: test\nNo closing delimiter";
        let result = parse_frontmatter::<TestFrontmatter>(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_body() {
        let content = r#"---
name: minimal
---
"#;

        let doc = parse_frontmatter::<TestFrontmatter>(content).unwrap();
        assert_eq!(doc.frontmatter.name, "minimal");
        assert!(doc.body.is_empty());
    }
}
