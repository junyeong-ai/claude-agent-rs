//! AskUserQuestionTool - interactive user prompts.
//!
//! This tool allows agents to ask users questions during execution,
//! gathering preferences, clarifications, and decisions.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::tools::{Tool, ToolResult};

/// Tool for asking users questions interactively
pub struct AskUserQuestionTool;

impl AskUserQuestionTool {
    /// Create a new AskUserQuestionTool
    pub fn new() -> Self {
        Self
    }
}

impl Default for AskUserQuestionTool {
    fn default() -> Self {
        Self::new()
    }
}

/// A single option for a question
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    /// Display text for this option
    pub label: String,
    /// Explanation of what this option means
    #[serde(default)]
    pub description: Option<String>,
}

impl QuestionOption {
    /// Create a new option with just a label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
        }
    }

    /// Create an option with label and description
    pub fn with_description(label: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: Some(description.into()),
        }
    }
}

/// A question to ask the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    /// The question text
    pub question: String,
    /// Short header/label for the question (max 12 chars)
    pub header: String,
    /// Available options (2-4 options)
    pub options: Vec<QuestionOption>,
    /// Whether multiple selections are allowed
    #[serde(default, rename = "multiSelect")]
    pub multi_select: bool,
}

impl Question {
    /// Create a new single-choice question
    pub fn single_choice(
        question: impl Into<String>,
        header: impl Into<String>,
        options: Vec<QuestionOption>,
    ) -> Self {
        Self {
            question: question.into(),
            header: header.into(),
            options,
            multi_select: false,
        }
    }

    /// Create a new multi-choice question
    pub fn multi_choice(
        question: impl Into<String>,
        header: impl Into<String>,
        options: Vec<QuestionOption>,
    ) -> Self {
        Self {
            question: question.into(),
            header: header.into(),
            options,
            multi_select: true,
        }
    }
}

/// Input for AskUserQuestion tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskUserQuestionInput {
    /// Questions to ask (1-4)
    pub questions: Vec<Question>,
    /// Previously collected answers (for follow-up questions)
    #[serde(default)]
    pub answers: Option<serde_json::Map<String, serde_json::Value>>,
}


#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        "AskUserQuestion"
    }

    fn description(&self) -> &str {
        "Ask the user questions during execution. Use to gather preferences, clarify \
         ambiguous instructions, or get decisions on implementation choices. \
         Users can always select 'Other' to provide custom input."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "description": "Questions to ask the user (1-4 questions)",
                    "minItems": 1,
                    "maxItems": 4,
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The complete question to ask"
                            },
                            "header": {
                                "type": "string",
                                "description": "Short label (max 12 chars)",
                                "maxLength": 12
                            },
                            "options": {
                                "type": "array",
                                "description": "Available choices (2-4 options)",
                                "minItems": 2,
                                "maxItems": 4,
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Display text for the option"
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "Explanation of what this option means"
                                        }
                                    },
                                    "required": ["label"]
                                }
                            },
                            "multiSelect": {
                                "type": "boolean",
                                "description": "Allow multiple selections",
                                "default": false
                            }
                        },
                        "required": ["question", "header", "options"]
                    }
                },
                "answers": {
                    "type": "object",
                    "description": "Previously collected answers"
                }
            },
            "required": ["questions"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: AskUserQuestionInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        // Validate input
        if input.questions.is_empty() || input.questions.len() > 4 {
            return ToolResult::error("Must provide 1-4 questions");
        }

        for q in &input.questions {
            if q.options.len() < 2 || q.options.len() > 4 {
                return ToolResult::error(format!(
                    "Question '{}' must have 2-4 options, got {}",
                    q.header,
                    q.options.len()
                ));
            }
            if q.header.len() > 12 {
                return ToolResult::error(format!(
                    "Header '{}' exceeds 12 character limit",
                    q.header
                ));
            }
        }

        // In a full implementation, this would:
        // 1. Present the questions to the user through the UI
        // 2. Wait for user response
        // 3. Return the selected options

        // For now, return a placeholder indicating questions were presented
        let question_summaries: Vec<String> = input
            .questions
            .iter()
            .map(|q| {
                let options: Vec<&str> = q.options.iter().map(|o| o.label.as_str()).collect();
                format!("{}: {} [{}]", q.header, q.question, options.join(", "))
            })
            .collect();

        ToolResult::success(format!(
            "Questions presented to user:\n{}\n\n\
             (User interaction is not yet implemented. \
             In production, this would wait for user response.)",
            question_summaries.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_question_option() {
        let opt = QuestionOption::with_description("OAuth", "Use OAuth 2.0 authentication");
        assert_eq!(opt.label, "OAuth");
        assert!(opt.description.is_some());
    }

    #[test]
    fn test_question_creation() {
        let q = Question::single_choice(
            "Which auth method?",
            "Auth",
            vec![
                QuestionOption::new("JWT"),
                QuestionOption::new("OAuth"),
            ],
        );
        assert!(!q.multi_select);
        assert_eq!(q.options.len(), 2);

        let q2 = Question::multi_choice(
            "Which features?",
            "Features",
            vec![
                QuestionOption::new("Logging"),
                QuestionOption::new("Metrics"),
                QuestionOption::new("Tracing"),
            ],
        );
        assert!(q2.multi_select);
    }

    #[tokio::test]
    async fn test_ask_user_question_tool() {
        let tool = AskUserQuestionTool::new();
        let result = tool
            .execute(serde_json::json!({
                "questions": [{
                    "question": "Which database should we use?",
                    "header": "Database",
                    "options": [
                        {"label": "PostgreSQL", "description": "Recommended for production"},
                        {"label": "SQLite", "description": "Good for development"}
                    ],
                    "multiSelect": false
                }]
            }))
            .await;

        assert!(!result.is_error());
    }

    #[tokio::test]
    async fn test_ask_user_question_validation() {
        let tool = AskUserQuestionTool::new();

        // Test too few options
        let result = tool
            .execute(serde_json::json!({
                "questions": [{
                    "question": "Single option?",
                    "header": "Test",
                    "options": [{"label": "Only one"}]
                }]
            }))
            .await;
        assert!(result.is_error());

        // Test header too long
        let result = tool
            .execute(serde_json::json!({
                "questions": [{
                    "question": "Question?",
                    "header": "ThisHeaderIsTooLong",
                    "options": [
                        {"label": "A"},
                        {"label": "B"}
                    ]
                }]
            }))
            .await;
        assert!(result.is_error());
    }
}
