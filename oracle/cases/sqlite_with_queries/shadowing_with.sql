with categories as (select id, name from main.categories where parent_id is not null) select id, name from categories
