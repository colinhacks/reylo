create macro slug(s) as lower(trim(s)); create table pages (id integer primary key, parent_id integer references pages (id), slug varchar check (slug <> ''));
