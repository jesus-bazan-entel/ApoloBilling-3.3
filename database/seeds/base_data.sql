--
-- PostgreSQL database dump
--

\restrict zEF3hMdmZOogyVecptIvvENUjIl3w85rcLKTcR2iKYpevO0w008gY5X9rvXzs63

-- Dumped from database version 15.16 (Debian 15.16-0+deb12u1)
-- Dumped by pg_dump version 15.16 (Debian 15.16-0+deb12u1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Data for Name: freeswitch_allowed_ips; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.freeswitch_allowed_ips (id, ip_address, description, enabled, created_at, updated_at) FROM stdin;
1	127.0.0.1	Localhost IPv4	t	2026-03-21 11:03:10.145335-07	2026-03-21 11:03:10.145335-07
2	::1	Localhost IPv6	t	2026-03-21 11:03:10.145335-07	2026-03-21 11:03:10.145335-07
3	10.10.22.4	Local FreeSWITCH Server	t	2026-03-21 11:22:17.371415-07	2026-03-21 11:22:17.371415-07
4	10.124.194.14	Internal Network Interface	t	2026-03-21 11:22:17.371415-07	2026-03-21 11:22:17.371415-07
5	10.124.194.142	Internal Network Interface 2	t	2026-03-21 11:22:17.371415-07	2026-03-21 11:22:17.371415-07
\.


--
-- Data for Name: plans; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.plans (id, plan_name, plan_code, account_type, initial_balance, credit_limit, max_concurrent_calls, description, enabled, created_at, updated_at, created_by) FROM stdin;
8	POSTPAGO_100_SOLES	PL100	POSTPAID	0.0000	100.0000	5	Plan	t	2026-02-03 07:48:28.05226-08	2026-03-21 11:55:12.510768-07	admin
\.


--
-- Data for Name: system_endpoints; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_endpoints (id, name, endpoint_type, description, ip_address, port, transport, fs_profile, fs_context, kam_gwid, kam_gw_type, enabled, is_primary, created_at, updated_at) FROM stdin;
c0816ff0-921c-413c-822a-ec76aff7dbc4	FreeSWITCH Internal	freeswitch	FreeSWITCH internal profile for registered devices	10.10.22.4	5080	udp	internal	from-pbx	\N	9	t	t	2026-04-08 18:48:51.442571-07	2026-04-08 18:48:51.442571-07
b982fcf8-90e6-41a7-aef3-dd788c99aa9e	FreeSWITCH External	freeswitch	FreeSWITCH external profile for trunk connections	10.10.22.4	5062	udp	external	public	\N	9	t	f	2026-04-08 18:48:51.442571-07	2026-04-08 18:48:51.442571-07
fd17ef02-4bba-439a-a9a4-8bb203714bbf	Kamailio SBC	kamailio	Kamailio SBC for carrier routing	10.10.22.4	5060	udp	\N	\N	\N	8	t	t	2026-04-08 18:48:51.443401-07	2026-04-08 18:48:51.443401-07
a72e1965-1b74-4a2c-9c09-083280829d4a	Kamailio PBX Endpoint	kamailio	Kamailio endpoint receiving calls from FreeSWITCH	10.10.22.4	5082	udp	\N	\N	100	9	t	f	2026-04-08 18:48:51.443401-07	2026-04-08 18:48:51.443401-07
d23be433-773e-44be-ae12-c0acb9f1ef99	FreeSWITCH Inbound	freeswitch	FreeSWITCH inbound profile for carrier calls	10.10.22.4	5082	udp	inbound	public	\N	9	t	f	2026-04-08 20:13:40.909547-07	2026-04-08 20:13:40.909547-07
\.


--
-- Data for Name: system_settings; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.system_settings (key, value, description, updated_at, updated_by) FROM stdin;
require_outbound_authorization	false	Require account authorization and billing for outbound calls. Set to false to allow all outbound calls without verification.	2026-03-15 20:18:59.897389-07	system
\.


--
-- Data for Name: zonas; Type: TABLE DATA; Schema: public; Owner: -
--

COPY public.zonas (id, zone_name, zone_code, zone_type, network_type, region_name, description, enabled, created_at, updated_at) FROM stdin;
\.


--
-- Name: freeswitch_allowed_ips_id_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.freeswitch_allowed_ips_id_seq', 5, true);


--
-- Name: plans_id_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.plans_id_seq', 8, true);


--
-- Name: zonas_id_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.zonas_id_seq', 1, false);


--
-- PostgreSQL database dump complete
--

\unrestrict zEF3hMdmZOogyVecptIvvENUjIl3w85rcLKTcR2iKYpevO0w008gY5X9rvXzs63

