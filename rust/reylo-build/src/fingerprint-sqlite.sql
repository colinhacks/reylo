select type, name, tbl_name, coalesce(sql, '') from sqlite_schema
    union all select 'function', name, type, narg from pragma_function_list order by 1, 2, 3, 4