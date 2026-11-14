create macro clean (xs) as (list_apply(list_filter(list_apply(xs, (lambda item: struct_pack("filter" := (item != ''), result := item))), (lambda elem: struct_extract(elem, 'filter'))), (lambda elem: struct_extract(elem, 'result'))));
create table orders (id integer, xs varchar[], n integer generated always as (len(clean(xs))) virtual, primary key (id));
create table item (id integer, order_id integer, primary key (id), foreign key (order_id) references orders (id));
