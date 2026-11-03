create type post_status as enum ('draft', 'published', 'archived');
alter table posts add column published_at timestamp with time zone;
alter table posts add column status post_status default 'draft' not null;
alter table posts add column tags text[] default '{}' not null;
alter table profiles add column deleted_at timestamp with time zone;
alter table profiles add column handle character varying(30);
alter table profiles add column updated_at timestamp with time zone;
alter table posts add constraint published_has_date check (status <> 'published' or published_at is not null);
alter table profiles add constraint profiles_handle_key unique (handle);
alter table profiles add constraint profiles_name_check check (char_length(name) between 1 and 100);
create index posts_author on posts (author) where status <> 'archived';
create index posts_search on posts using gin (to_tsvector('english', title || ' ' || body));
create index posts_tags on posts using gin (tags);
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
