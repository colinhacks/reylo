select o.a, o.b from js cross apply openjson(js.j) with (a int '$.a', b int '$.b') o
