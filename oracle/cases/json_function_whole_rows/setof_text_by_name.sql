select to_jsonb(many_bodies.*) as doc from many_bodies(:p::int8)
