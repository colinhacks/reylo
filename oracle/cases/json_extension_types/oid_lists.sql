select json_agg(o) as os, jsonb_agg(l) as ls from blobs
