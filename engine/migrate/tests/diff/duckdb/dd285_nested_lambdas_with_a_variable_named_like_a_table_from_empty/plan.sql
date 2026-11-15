create macro flat (xss) as (list_transform(xss, (lambda xs: list_transform(xs, (lambda tag: (tag || ''))))));
create table posts (id integer, xs varchar[][], f varchar[][] generated always as (flat(xs)) virtual, primary key (id));
create table tag (id integer, post_id integer, primary key (id), foreign key (post_id) references posts (id));
