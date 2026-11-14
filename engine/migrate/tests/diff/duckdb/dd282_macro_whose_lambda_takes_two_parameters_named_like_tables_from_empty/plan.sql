create macro idx (xs) as (list_transform(xs, (lambda child, parent: (child || cast(parent as VARCHAR)))));
create table parent (id integer, tags varchar[], s varchar[] generated always as (idx(tags)) virtual, primary key (id));
create table child (id integer, parent_id integer, primary key (id), foreign key (parent_id) references parent (id));
