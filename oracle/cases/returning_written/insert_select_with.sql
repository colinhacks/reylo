with s as (select 10 as id, 'w' as note) insert into t (id, note) select id, note from s returning note
