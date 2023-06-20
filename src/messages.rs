use itertools::Itertools;

pub fn creat_review_group_init_message(testers: &[String], publisher: &String) -> String {
    format!(
        "Publisher: [{publisher}] \nTesters: [{}]",
        testers.iter().join(",")
    )
}

pub fn appstore_message() -> &'static str {
    r#"Welcome to the xdcstore!"#
}
