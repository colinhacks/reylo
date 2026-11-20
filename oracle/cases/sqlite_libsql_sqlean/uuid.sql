select uuid4() as a, uuid7() as b, gen_random_uuid() as c, uuid_str(uuid4()) as d, uuid_blob(uuid7()) as e, uuid7_timestamp_ms(uuid7()) as f
