create table posts (id int primary key, old_title text);
create table drafts (id int primary key);
grant update (old_title) on posts to 'reader'@'%';
grant select on drafts to 'reader'@'%';
