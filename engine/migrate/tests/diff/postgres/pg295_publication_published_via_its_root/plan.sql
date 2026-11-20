drop publication everything;
create publication everything for table p;
alter publication root set (publish = 'insert, update, delete, truncate', publish_via_partition_root = true);
