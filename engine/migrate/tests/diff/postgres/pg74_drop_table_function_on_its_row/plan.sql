-- loses data: bookings
drop index bookings_priority;
alter table bookings drop constraint bookings_priority_check;
drop function booking_priority(bookings);
drop table bookings;
drop type booking_status;
