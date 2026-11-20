create publication everything for all tables;
alter publication live set table only messages (id, room) where ((room <> 'secret'::text)), table only rooms;
alter publication live set (publish = 'insert, update', publish_via_partition_root = false);
