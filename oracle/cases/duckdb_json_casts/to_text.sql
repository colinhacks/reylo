select body::varchar as a, cast(body as text) as b, body::json as c, typed::varchar as d from docs
