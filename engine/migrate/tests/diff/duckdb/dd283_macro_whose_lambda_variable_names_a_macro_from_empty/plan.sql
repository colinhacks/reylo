create macro mk (s) as ((s || '!'));
create macro bang (xs) as (list_transform(xs, (lambda mk: mk(mk))));
create table t (id integer, tags varchar[], b varchar[] generated always as (bang(tags)) virtual, primary key (id));
