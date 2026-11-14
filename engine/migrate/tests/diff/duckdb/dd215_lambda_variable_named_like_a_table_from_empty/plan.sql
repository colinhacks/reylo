create macro clean (xs) as (list_filter(xs, (lambda tag: (tag != ''))));
create table posts (id integer, xs varchar[], n integer generated always as (len(clean(xs))) virtual, primary key (id));
create table tag (id integer, post_id integer, name varchar, primary key (id), foreign key (post_id) references posts (id));
