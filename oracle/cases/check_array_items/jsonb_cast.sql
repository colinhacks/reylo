-- A cast to a number array makes a JSON null element NULL, so the CHECK's non-null elements hold
-- for the jsonb array alone, coalesced or not.
select a::int[] as bare, coalesce(a, '{}')::int[] as coalesced, coalesce(a, '{}') as kept from documents
