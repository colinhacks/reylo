select jsonb_agg(location) as locations, json_agg(shape) as shapes from places
