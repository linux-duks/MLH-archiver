use mlh_archiver::config::{AppConfig, RunMode};
use mlh_archiver::errors::ConfigError;
use mlh_archiver::nntp_source::nntp_config::NntpConfig;

// =============================================================================
// AppConfig Deserialization Tests
// =============================================================================

#[test]
fn test_app_config_defaults() {
    let config = AppConfig::default();
    assert_eq!(config.nthreads, 1);
    assert_eq!(config.output_dir, "./output");
    assert!(config.loop_groups);
    assert!(config.nntp.is_none());
}

#[test]
fn test_app_config_deserialize_nested_format() {
    let yaml = r#"
nthreads: 4
output_dir: "./custom_output"
loop_groups: false
nntp:
  hostname: "nntp.example.com"
  port: 563
  group_lists: ["list1", "list2"]
"#;
    let config: AppConfig = serde_yaml::from_str(yaml).expect("Failed to parse");
    assert_eq!(config.nthreads, 4);
    assert_eq!(config.output_dir, "./custom_output");
    assert!(!config.loop_groups);
    assert!(config.nntp.is_some());
    let nntp = config.nntp.unwrap();
    assert_eq!(nntp.hostname, "nntp.example.com");
    assert_eq!(nntp.port, 563);
    assert_eq!(
        nntp.group_lists,
        Some(vec!["list1".to_string(), "list2".to_string()])
    );
}

#[test]
fn test_app_config_deserialize_defaults() {
    let yaml = r#"
nntp:
  hostname: "nntp.example.com"
"#;
    let config: AppConfig = serde_yaml::from_str(yaml).expect("Failed to parse");
    assert_eq!(config.nthreads, 1);
    assert_eq!(config.output_dir, "./output");
    assert!(config.loop_groups);
}

#[test]
fn test_app_config_deserialize_missing_nntp() {
    let yaml = r#"
nthreads: 2
output_dir: "./test"
"#;
    let config: AppConfig = serde_yaml::from_str(yaml).expect("Failed to parse");
    assert!(config.nntp.is_none());
}

#[test]
fn test_app_config_deserialize_invalid_port_type() {
    let yaml = r#"
nntp:
  hostname: "nntp.example.com"
  port: "not_a_number"
"#;
    let result: Result<AppConfig, _> = serde_yaml::from_str(yaml);
    assert!(result.is_err());
}

#[test]
fn test_nntp_config_default_port() {
    let yaml = r#"
nntp:
  hostname: "nntp.example.com"
"#;
    let config: AppConfig = serde_yaml::from_str(yaml).expect("Failed to parse");
    assert_eq!(config.nntp.as_ref().unwrap().port, 119);
}

// =============================================================================
// NntpConfig Tests
// =============================================================================

#[test]
fn test_nntp_config_validate_success() {
    let config = NntpConfig {
        hostname: "nntp.example.com".to_string(),
        port: 119,
        group_lists: None,
        article_range: None,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_nntp_config_validate_missing_hostname() {
    let config = NntpConfig {
        hostname: String::new(),
        port: 119,
        group_lists: None,
        article_range: None,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConfigError::MissingHostname));
}

#[test]
fn test_nntp_config_server_address_default_port() {
    let config = NntpConfig {
        hostname: "nntp.example.com".to_string(),
        port: 119,
        group_lists: None,
        article_range: None,
    };
    assert_eq!(config.server_address(), "nntp.example.com:119");
}

#[test]
fn test_nntp_config_server_address_custom_port() {
    let config = NntpConfig {
        hostname: "nntp.example.com".to_string(),
        port: 563,
        group_lists: None,
        article_range: None,
    };
    assert_eq!(config.server_address(), "nntp.example.com:563");
}

// =============================================================================
// AppConfig Methods Tests
// =============================================================================

#[test]
fn test_app_config_get_nntp_config_with_nntp() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: None,
            article_range: None,
        }),
    };
    let nntp = config.nntp.unwrap();
    assert_eq!(nntp.hostname, "nntp.example.com");
}

#[test]
fn test_app_config_get_nntp_config_without_nntp() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: None,
    };
    assert!(config.nntp.is_none());
}

// =============================================================================
// get_group_lists() Tests
// =============================================================================

#[test]
fn test_get_group_lists_all_keyword() {
    let mut config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["ALL".to_string()]),
            article_range: None,
        }),
    };

    let available_lists = vec![
        "list1".to_string(),
        "list2".to_string(),
        "list3".to_string(),
    ];

    let result = config.get_group_lists(
        available_lists.clone(),
        RunMode::NNTP,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), available_lists);
}

#[test]
fn test_get_group_lists_specific_lists() {
    let mut config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["list1".to_string(), "list2".to_string()]),
            article_range: None,
        }),
    };

    let available_lists = vec![
        "list1".to_string(),
        "list2".to_string(),
        "list3".to_string(),
    ];

    let result = config.get_group_lists(
        available_lists,
        RunMode::NNTP,
    );
    assert!(result.is_ok());
    let lists = result.unwrap();
    assert_eq!(lists.len(), 2);
    assert!(lists.contains(&"list1".to_string()));
    assert!(lists.contains(&"list2".to_string()));
}

#[test]
fn test_get_group_lists_filters_invalid() {
    let mut config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["valid_list".to_string(), "invalid_list".to_string()]),
            article_range: None,
        }),
    };

    let available_lists = vec!["valid_list".to_string(), "another_valid_list".to_string()];

    let result = config.get_group_lists(
        available_lists,
        RunMode::NNTP,
    );
    assert!(result.is_ok());
    let lists = result.unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0], "valid_list");
}

#[test]
fn test_get_group_lists_all_invalid() {
    let mut config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["invalid1".to_string(), "invalid2".to_string()]),
            article_range: None,
        }),
    };

    let available_lists = vec!["valid_list".to_string(), "another_valid_list".to_string()];

    let result = config.get_group_lists(
        available_lists,
        RunMode::NNTP,
    );
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ConfigError::AllListsUnavailable
    ));
}

#[test]
fn test_get_group_lists_deduplicates() {
    let mut config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec![
                "list1".to_string(),
                "list1".to_string(),
                "list2".to_string(),
            ]),
            article_range: None,
        }),
    };

    let available_lists = vec!["list1".to_string(), "list2".to_string()];

    let result = config.get_group_lists(
        available_lists,
        RunMode::NNTP,
    );
    assert!(result.is_ok());
    let lists = result.unwrap();
    assert_eq!(lists.len(), 2);
}

// =============================================================================
// get_article_range() Tests
// =============================================================================

#[test]
fn test_get_article_range_none() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: None,
            article_range: None,
        }),
    };

    let result = config.get_range_selection_text(RunMode::NNTP);
    assert!(result.is_none());
}

#[test]
fn test_get_article_range_single_number() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["list1".to_string()]),
            article_range: Some("100".to_string()),
        }),
    };

    let result = config.get_range_selection_text(RunMode::NNTP);
    assert!(result.is_some());
    let range: Vec<usize> = mlh_archiver::range_inputs::parse_sequence(&result.unwrap()).unwrap().collect();
    assert_eq!(range, vec![100]);
}

#[test]
fn test_get_article_range_multiple_numbers() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["list1".to_string()]),
            article_range: Some("1,5,10".to_string()),
        }),
    };

    let result = config.get_range_selection_text(RunMode::NNTP);
    assert!(result.is_some());
    let range: Vec<usize> = mlh_archiver::range_inputs::parse_sequence(&result.unwrap()).unwrap().collect();
    assert_eq!(range, vec![1, 5, 10]);
}

#[test]
fn test_get_article_range_dash_range() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["list1".to_string()]),
            article_range: Some("1-5".to_string()),
        }),
    };

    let result = config.get_range_selection_text(RunMode::NNTP);
    assert!(result.is_some());
    let range: Vec<usize> = mlh_archiver::range_inputs::parse_sequence(&result.unwrap()).unwrap().collect();
    assert_eq!(range, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_get_article_range_mixed() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["list1".to_string()]),
            article_range: Some("1,3-5,10".to_string()),
        }),
    };

    let result = config.get_range_selection_text(RunMode::NNTP);
    assert!(result.is_some());
    let range: Vec<usize> = mlh_archiver::range_inputs::parse_sequence(&result.unwrap()).unwrap().collect();
    assert_eq!(range, vec![1, 3, 4, 5, 10]);
}

#[test]
fn test_get_article_range_invalid() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: "nntp.example.com".to_string(),
            port: 119,
            group_lists: Some(vec!["list1".to_string()]),
            article_range: Some("invalid".to_string()),
        }),
    };

    // get_range_selection_text returns the raw string
    let result = config.get_range_selection_text(RunMode::NNTP);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "invalid");
    
    // But parsing it fails
    let parsed = mlh_archiver::range_inputs::parse_sequence("invalid");
    assert!(parsed.is_err());
}

#[test]
fn test_get_article_range_no_nntp() {
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: None,
    };

    // When nntp is None, get_range_selection_text returns None
    let result = config.get_range_selection_text(RunMode::NNTP);
    assert!(result.is_none());
}

// =============================================================================
// Integration Tests
// =============================================================================

#[test]
fn test_full_config_workflow() {
    let yaml = r#"
nthreads: 3
output_dir: "./integration_test_output"
loop_groups: true
nntp:
  hostname: "nntp.example.com"
  port: 119
  group_lists: ["list1", "list2"]
  article_range: "1-10"
"#;

    let mut config: AppConfig = serde_yaml::from_str(yaml).expect("Failed to parse");

    // Verify parsed values
    assert_eq!(config.nthreads, 3);
    assert_eq!(config.output_dir, "./integration_test_output");
    assert!(config.loop_groups);
    assert!(config.nntp.is_some());

    // Verify article range parsing
    let range = config.get_range_selection_text(RunMode::NNTP);
    assert!(range.is_some());
    let range_vec: Vec<usize> = mlh_archiver::range_inputs::parse_sequence(&range.unwrap()).unwrap().collect();
    assert_eq!(range_vec, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

    // Verify group lists
    let available_lists = vec![
        "list1".to_string(),
        "list2".to_string(),
        "list3".to_string(),
    ];
    let groups = config.get_group_lists(
        available_lists,
        RunMode::NNTP,
    );
    assert!(groups.is_ok());
    assert_eq!(groups.unwrap().len(), 2);
}

#[test]
fn test_config_validation_workflow() {
    // Test that validation catches missing hostname
    let config = AppConfig {
        nthreads: 1,
        output_dir: "./output".to_string(),
        loop_groups: true,
        nntp: Some(NntpConfig {
            hostname: String::new(),
            port: 119,
            group_lists: None,
            article_range: None,
        }),
    };

    if let Some(ref nntp) = config.nntp {
        let result = nntp.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::MissingHostname));
    }
}
