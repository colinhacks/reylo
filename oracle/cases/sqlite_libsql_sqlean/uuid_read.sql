select uuid_str(nick) as a, uuid_blob(name) as b, uuid7_timestamp_ms(bio) as c, uuid_str(:id) as d from people
