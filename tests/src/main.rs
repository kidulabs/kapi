use kapi_tests::{
    TestApp, all_test_stores, generation_semantics, list_filtering, object_annotations,
    object_crud, object_labels, optimistic_concurrency, schema_deletion, schema_validation,
    status_subresource, watch_events,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("RUST_LOG"))
        .init();

    let stores = all_test_stores();
    let mut overall_failed = false;

    for store in &stores {
        println!("=== {} ===", store.name);
        println!();

        macro_rules! run_test {
            ($name:expr, $test:expr) => {{
                print!("  {} ... ", $name);
                let app = TestApp::with_store((store.factory)());
                match $test(&app).await {
                    Ok(()) => println!("ok"),
                    Err(e) => {
                        println!("FAILED");
                        println!("    {e}");
                        overall_failed = true;
                    }
                }
            }};
        }

        run_test!(
            "create_schema_then_object",
            object_crud::test_create_schema_then_object
        );
        run_test!("full_crud_flow", object_crud::test_full_crud_flow);
        run_test!("list_single_page", object_crud::test_list_single_page);
        run_test!("list_two_pages", object_crud::test_list_two_pages);
        run_test!(
            "list_resume_position",
            object_crud::test_list_resume_position
        );
        run_test!("list_exhausted", object_crud::test_list_exhausted);

        println!();
        run_test!("create_missing_spec", object_crud::test_create_missing_spec);
        run_test!("create_empty_spec", object_crud::test_create_empty_spec);
        run_test!(
            "create_non_object_spec",
            object_crud::test_create_non_object_spec
        );
        run_test!(
            "create_unknown_top_level_field",
            object_crud::test_create_unknown_top_level_field
        );

        println!();
        run_test!(
            "create_object_with_labels",
            object_labels::test_create_object_with_labels
        );
        run_test!(
            "create_object_without_labels",
            object_labels::test_create_object_without_labels
        );
        run_test!(
            "update_object_labels",
            object_labels::test_update_object_labels
        );
        run_test!(
            "create_schema_with_labels",
            object_labels::test_create_schema_with_labels
        );
        run_test!(
            "invalid_label_key_400",
            object_labels::test_invalid_label_key_format
        );
        run_test!(
            "invalid_label_value_400",
            object_labels::test_invalid_label_value_format
        );
        run_test!(
            "label_key_too_long_400",
            object_labels::test_label_key_exceeds_length
        );
        run_test!(
            "label_value_too_long_400",
            object_labels::test_label_value_exceeds_length
        );

        println!();
        run_test!(
            "create_object_with_annotations",
            object_annotations::test_create_object_with_annotations
        );
        run_test!(
            "create_object_without_annotations",
            object_annotations::test_create_object_without_annotations
        );
        run_test!(
            "update_object_annotations",
            object_annotations::test_update_object_annotations
        );
        run_test!(
            "create_schema_with_annotations",
            object_annotations::test_create_schema_with_annotations
        );
        run_test!(
            "invalid_annotation_key_empty",
            object_annotations::test_invalid_annotation_key_empty
        );
        run_test!(
            "invalid_annotation_key_too_long",
            object_annotations::test_invalid_annotation_key_too_long
        );
        run_test!(
            "invalid_annotation_value_non_string",
            object_annotations::test_invalid_annotation_value_non_string
        );
        run_test!(
            "invalid_annotations_format",
            object_annotations::test_invalid_annotations_format
        );
        run_test!(
            "annotation_size_limit",
            object_annotations::test_annotation_size_limit
        );
        run_test!(
            "annotation_size_limit_on_update",
            object_annotations::test_annotation_size_limit_on_update
        );
        run_test!("watch_schema_added", watch_events::test_watch_schema_added);
        run_test!(
            "watch_object_events",
            watch_events::test_watch_object_events
        );
        run_test!(
            "watch_by_name_matching",
            watch_events::test_watch_by_name_matching_events
        );
        run_test!(
            "watch_by_name_non_matching_filtered",
            watch_events::test_watch_by_name_non_matching_filtered
        );
        run_test!(
            "watch_invalid_field_selector",
            watch_events::test_watch_invalid_field_selector
        );
        run_test!(
            "watch_by_name_and_all_simultaneous",
            watch_events::test_watch_by_name_and_watch_all_simultaneously
        );
        run_test!(
            "watcher_cleanup_on_disconnect",
            watch_events::test_watcher_cleanup_on_client_disconnect
        );

        println!();
        run_test!(
            "list_with_field_selector",
            list_filtering::test_list_with_field_selector
        );
        run_test!(
            "list_with_label_selector",
            list_filtering::test_list_with_label_selector
        );
        run_test!(
            "list_with_both_selectors",
            list_filtering::test_list_with_both_selectors
        );
        run_test!(
            "list_filter_with_pagination",
            list_filtering::test_list_filter_with_pagination
        );
        run_test!(
            "list_filter_no_matches",
            list_filtering::test_list_filter_no_matches
        );
        run_test!(
            "watch_with_both_selectors_matching",
            list_filtering::test_watch_with_both_selectors_matching
        );
        run_test!(
            "watch_with_both_selectors_not_matching",
            list_filtering::test_watch_with_both_selectors_not_matching
        );
        run_test!(
            "list_invalid_field_selector",
            list_filtering::test_list_invalid_field_selector
        );
        run_test!(
            "list_invalid_label_selector",
            list_filtering::test_list_invalid_label_selector
        );

        println!();
        run_test!(
            "watch_by_label_selector_matching",
            watch_events::test_watch_by_label_selector_matching
        );
        run_test!(
            "watch_by_label_selector_non_matching",
            watch_events::test_watch_by_label_selector_non_matching
        );
        run_test!(
            "watch_by_label_selector_and_combinator",
            watch_events::test_watch_by_label_selector_and_combinator
        );
        run_test!(
            "watch_by_label_selector_not_exists",
            watch_events::test_watch_by_label_selector_not_exists
        );
        run_test!(
            "watch_invalid_label_selector",
            watch_events::test_watch_invalid_label_selector
        );
        run_test!(
            "watch_empty_label_selector",
            watch_events::test_watch_empty_label_selector
        );

        println!();
        run_test!(
            "delete_schema_no_objects",
            schema_deletion::test_delete_schema_no_objects
        );
        run_test!(
            "delete_schema_with_objects",
            schema_deletion::test_delete_schema_with_objects
        );

        println!();
        run_test!(
            "valid_schema_accepted",
            schema_validation::test_valid_schema_accepted
        );
        run_test!(
            "invalid_spec_schema_rejected",
            schema_validation::test_invalid_spec_schema_rejected
        );
        run_test!(
            "missing_required_fields_rejected",
            schema_validation::test_missing_required_fields_rejected
        );

        println!();
        run_test!(
            "update_correct_rv",
            optimistic_concurrency::test_update_correct_rv
        );
        run_test!(
            "update_wrong_rv_returns_conflict",
            optimistic_concurrency::test_update_wrong_rv_returns_conflict
        );

        println!();

        run_test!(
            "generation_semantics",
            generation_semantics::test_generation_semantics
        );

        println!();

        // Status subresource tests
        println!();
        run_test!(
            "status_update_with_schema",
            status_subresource::test_status_subresource_update
        );
        run_test!(
            "status_not_enabled",
            status_subresource::test_status_subresource_not_enabled
        );
        run_test!(
            "status_invalid_data",
            status_subresource::test_status_subresource_invalid_data
        );
        run_test!(
            "concurrent_spec_and_status",
            status_subresource::test_concurrent_spec_and_status_update
        );
        run_test!(
            "create_rejects_unknown_fields",
            status_subresource::test_create_rejects_unknown_top_level_fields
        );
        run_test!(
            "status_update_nonexistent",
            status_subresource::test_status_update_nonexistent_object
        );
        run_test!(
            "status_modified_event",
            status_subresource::test_status_update_publishes_status_modified_event
        );
        run_test!(
            "status_preserves_spec",
            status_subresource::test_status_update_preserves_spec
        );
        run_test!(
            "status_bumps_rv",
            status_subresource::test_status_update_bumps_resource_version
        );
        run_test!(
            "invalid_status_schema_rejected",
            status_subresource::test_invalid_status_schema_rejected
        );
        run_test!(
            "get_status_null_when_not_set",
            status_subresource::test_get_status_returns_null_when_not_set
        );
        run_test!(
            "meta_schema_rejects_invalid_status_schema",
            status_subresource::test_meta_schema_rejects_invalid_status_schema_type
        );
        run_test!(
            "status_replaces_not_merges",
            status_subresource::test_status_update_replaces_not_merges
        );
        run_test!(
            "spec_update_modified_event",
            status_subresource::test_spec_update_publishes_modified_not_status_modified
        );

        println!();

        if overall_failed {
            println!("FAILED: {} store", store.name);
            std::process::exit(1);
        }

        println!("passed: {} store", store.name);
        println!();
    }

    println!("all stores passed");
}
