create table t (id integer primary key, name varchar check (name <> ''), meta struct(a integer), tags varchar[],
        up varchar generated always as (upper(name)) virtual, ma integer generated always as (struct_extract(meta, 'a')) virtual,
        first varchar generated always as (tags[1]) virtual, sized varchar generated always as (case when len(tags) > 1 then 'many' else 'few' end) virtual,
        txt varchar generated always as (id::varchar || '-' || name) virtual);
