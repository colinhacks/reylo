select n from docs where json_merge_patch('{}', cast(n as char), '1') is not null
