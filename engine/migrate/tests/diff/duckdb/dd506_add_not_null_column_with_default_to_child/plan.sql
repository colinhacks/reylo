drop index posts_title;
alter table posts add column views integer not null default 0;
create index posts_title on posts(title);
