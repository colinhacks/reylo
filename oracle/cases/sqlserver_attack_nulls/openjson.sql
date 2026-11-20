select o.[key] as k, o.value as v, o.type as t from js cross apply openjson(js.j) o
