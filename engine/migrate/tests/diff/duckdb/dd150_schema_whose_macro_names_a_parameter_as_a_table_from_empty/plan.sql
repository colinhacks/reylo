create macro has_tag (tags, t) as (list_contains(tags, t));
create table posts (id integer, tags varchar[], primary key (id), check ((not has_tag(tags, 'spam'))));
create table tags (id integer, post_id integer, name varchar, primary key (id), foreign key (post_id) references posts (id));
