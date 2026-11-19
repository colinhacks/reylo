drop type numrange_custom;
drop type textrange;
create type floats as range (subtype = double precision, multirange_type_name = float_spans);
create type numrange_custom as range (subtype = numeric, multirange_type_name = numrange_custom_mr);
create type textrange as range (subtype = text, subtype_opclass = pg_catalog.text_pattern_ops);
