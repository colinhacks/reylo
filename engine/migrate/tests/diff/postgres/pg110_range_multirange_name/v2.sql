create table kept (id integer primary key);
create type textrange as range (subtype = text, subtype_opclass = text_pattern_ops);
create type numrange_custom as range (subtype = numeric, multirange_type_name = numrange_custom_mr);
create type floats as range (subtype = float8, multirange_type_name = float_spans);
