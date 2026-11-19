select (select json_agg(d) from doc d) as list
