select gen_random_uuid() as v4, uuidv4() as also_v4, uuidv7() as v7, uuidv7('-1 day'::interval) as shifted, id from made
