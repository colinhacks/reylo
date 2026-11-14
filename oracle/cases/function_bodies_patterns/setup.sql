-- Each pattern of query composition, written as SQL functions in the database, over the five tables
-- of the measured Kysely page. What a function's body says reaches the statements that call it.
create table person (id bigint primary key, first_name text not null, last_name text not null, email text not null, deleted_at timestamptz);
create table post (id bigint primary key, author_id bigint not null references person, title text not null, body text not null, status text not null check (status in ('draft', 'published')), published_at timestamptz, deleted_at timestamptz);
create table comment (id bigint primary key, post_id bigint not null references post, author_id bigint not null references person, body text not null, created_at timestamptz not null default now());
create table tag (id bigint primary key, name text not null unique);
create table post_tag (post_id bigint not null references post, tag_id bigint not null references tag, primary key (post_id, tag_id));

-- Filters as functions, combined into rules.
create function is_published(p post) returns boolean language sql stable as $$ select p.status = 'published' and p.deleted_at is null $$;
create function written_by(p post, author bigint) returns boolean language sql stable as $$ select p.author_id = author $$;
create function visible_to(p post, viewer bigint) returns boolean language sql stable as $$ select is_published(p) or written_by(p, viewer) $$;

-- One expression in every clause, and one that names no table.
create function full_name(p person) returns text language sql stable as $$ select p.first_name || ' ' || p.last_name $$;
create function matches(document text, query text) returns boolean language sql stable as $$ select to_tsvector('english', document) @@ websearch_to_tsquery('english', query) $$;

-- Relations as reusable selections, nested.
create function author_of(p post) returns jsonb language sql stable as $$ select jsonb_build_object('id', a.id, 'name', a.first_name || ' ' || a.last_name) from person a where a.id = p.author_id $$;
create function tags_of(p post) returns jsonb language sql stable as $$ select coalesce(jsonb_agg(jsonb_build_object('id', g.id, 'name', g.name) order by g.name), '[]'::jsonb) from tag g join post_tag l on l.tag_id = g.id where l.post_id = p.id $$;
create function commenter(c comment) returns jsonb language sql stable as $$ select jsonb_build_object('id', a.id, 'name', a.first_name || ' ' || a.last_name) from person a where a.id = c.author_id $$;
create function latest_comments(p post, n int default 2) returns jsonb language sql stable as $$ select coalesce(jsonb_agg(jsonb_build_object('id', x.id, 'body', x.body, 'by', commenter(x))), '[]'::jsonb) from (select c.* from comment c where c.post_id = p.id order by c.created_at desc limit n) x $$;
create function comment_count(p post) returns bigint language sql stable as $$ select count(*) from comment c where c.post_id = p.id $$;

-- Optional inputs, one base for a page and its total.
create function search_posts(q text default null, author bigint default null, tag_name text default null, since timestamptz default null) returns setof post language sql stable as $$
  select x.* from post x
  where is_published(x)
    and (q is null or matches(x.body, q))
    and (author is null or x.author_id = author)
    and (since is null or x.published_at >= since)
    and (tag_name is null or exists (select from post_tag l join tag g on g.id = l.tag_id where l.post_id = x.id and g.name = tag_name))
$$;

-- A key filter, and the shapes that write.
create function post_by_id(wanted bigint) returns setof post language sql stable as $$ select * from post where id = wanted $$;
create function publish(wanted bigint) returns table (id bigint, status text) language sql as $$ update post set status = 'published', published_at = now() where post.id = wanted returning post.id, post.status $$;
