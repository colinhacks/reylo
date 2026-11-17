select concat_ws(' ', @@version, @@sql_mode, count(*), sum(crc32(x)), bit_xor(crc32(x))) from (
    select concat_ws(' ', 'c', table_name, column_name, ordinal_position, column_type, coalesce(character_set_name, ''), is_nullable, column_key, extra, coalesce(generation_expression, ''), md5(column_comment)) as x from information_schema.columns where table_schema = database()
    union all select concat_ws(' ', 's', table_name, index_name, non_unique, seq_in_index, column_name) from information_schema.statistics where table_schema = database()
    union all select concat_ws(' ', 'v', table_name, md5(view_definition)) from information_schema.views where table_schema = database()
    union all select concat_ws(' ', 'r', routine_name, routine_type, coalesce(dtd_identifier, ''), md5(coalesce(routine_definition, ''))) from information_schema.routines where routine_schema = database()
    union all select concat_ws(' ', 'p', routine_schema, routine_name) from information_schema.routines where routine_schema <> database() and routine_type = 'FUNCTION'
    union all select concat_ws(' ', 'g', event_object_table, trigger_name, event_manipulation, action_timing) from information_schema.triggers where event_object_schema = database()
    union all select concat_ws(' ', 'f', table_name, constraint_name, referenced_table_name, update_rule, delete_rule) from information_schema.referential_constraints where constraint_schema = database()
    union all select concat_ws(' ', 'k', table_name, constraint_name, constraint_type, enforced) from information_schema.table_constraints where table_schema = database()
    union all select concat_ws(' ', 'h', constraint_name, md5(check_clause)) from information_schema.check_constraints where constraint_schema = database()
  ) t