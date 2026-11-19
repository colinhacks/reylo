-- file 1 of 8
set check_function_bodies = off;
create type post_status as enum ('draft', 'published', 'archived');
alter table posts add column published_at timestamp with time zone;
alter table posts add column status post_status default 'draft' not null;
alter table posts add column tags text[] default '{}' not null;
alter table profiles add column deleted_at timestamp with time zone;
alter table profiles add column handle character varying(30);
alter table profiles add column updated_at timestamp with time zone;
alter table posts add constraint published_has_date check (status <> 'published' or published_at is not null) not valid;
reset check_function_bodies;
-- file 2 of 8
create unique index concurrently profiles_handle_key on profiles (handle);
-- file 3 of 8
set check_function_bodies = off;
alter table profiles add constraint profiles_handle_key unique using index profiles_handle_key;
alter table profiles add constraint profiles_name_check check (char_length(name) between 1 and 100) not valid;
reset check_function_bodies;
-- file 4 of 8
create index concurrently posts_author on posts (author) where status <> 'archived';
-- file 5 of 8
create index concurrently posts_search on posts using gin (to_tsvector('english', title || ' ' || body));
-- file 6 of 8
create index concurrently posts_tags on posts using gin (tags);
-- file 7 of 8
set check_function_bodies = off;
create view published_posts with (security_invoker = true) as
  select p.id, p.title, a.name as author_name, p.published_at
  from posts p join profiles a on a.id = p.author
  where p.status = 'published' and a.deleted_at is null;
create function current_profile() returns uuid language sql stable as $$
  select nullif(current_setting('app.profile', true), '')::uuid
$$;
create function set_updated_at() returns trigger language plpgsql security definer set search_path = '' as $$
begin
  new.updated_at := now();
  return new;
end
$$;
create trigger profiles_updated_at before update on profiles for each row execute function set_updated_at();
create policy posts_read on posts for select using (status = 'published' or author = (select current_profile()));
create policy posts_write on posts for all using (author = (select current_profile())) with check (author = (select current_profile()));
alter table posts enable row level security;
comment on table posts is 'What profiles write';
reset check_function_bodies;
-- file 8 of 8
alter table posts validate constraint published_has_date;
alter table profiles validate constraint profiles_name_check;
