create publication everything for all tables;
alter publication live set table messages (id, room) where ((room <> 'secret'::text)), table rooms;
alter publication live set (publish = 'insert, update', publish_via_partition_root = false);
