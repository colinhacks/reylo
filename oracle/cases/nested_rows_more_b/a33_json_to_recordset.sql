select array(select f from json_to_recordset('[{"a":1},{"b":"x"}]') as f(a int, b text)) as arr
