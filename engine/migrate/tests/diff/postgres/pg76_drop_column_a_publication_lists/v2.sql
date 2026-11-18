create table lab_results (id bigint primary key);
create publication lab_feed for table lab_results (id);
