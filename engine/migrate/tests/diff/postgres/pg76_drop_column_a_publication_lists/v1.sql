create table lab_results (id bigint primary key, flash_summary text);
create publication lab_feed for table lab_results (id, flash_summary);
