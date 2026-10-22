with s as (select jsonb_build_object('id', id, 'title', title) as post from posts) select post from s
