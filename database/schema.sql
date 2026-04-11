--
-- PostgreSQL database dump
--

\restrict VZZ2gHDkXpogb5shkXx0YBn58SzcG4XFsLTEkqwumd27KR3jtZiBCMq2WWC8oCi

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

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: accounts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.accounts (
    id integer NOT NULL,
    account_number character varying(50) NOT NULL,
    account_name character varying(200),
    account_type character varying(20) DEFAULT 'PREPAID'::character varying NOT NULL,
    status character varying(20) DEFAULT 'ACTIVE'::character varying NOT NULL,
    balance numeric(12,4) DEFAULT 0.0000 NOT NULL,
    currency character varying(3) DEFAULT 'PEN'::character varying NOT NULL,
    credit_limit numeric(12,4) DEFAULT 0.0000,
    max_concurrent_calls integer DEFAULT 5,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    created_by character varying(100) DEFAULT 'system'::character varying,
    updated_by character varying(100) DEFAULT 'system'::character varying,
    plan_id integer,
    customer_phone character varying(50)
);


--
-- Name: COLUMN accounts.plan_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.accounts.plan_id IS 'Plan usado al crear la cuenta (nullable para cuentas existentes)';


--
-- Name: accounts_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.accounts_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: accounts_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.accounts_id_seq OWNED BY public.accounts.id;


--
-- Name: active_calls; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.active_calls (
    id integer NOT NULL,
    call_id character varying(100) NOT NULL,
    calling_number character varying(50),
    called_number character varying(50),
    direction character varying(20),
    start_time timestamp with time zone NOT NULL,
    answer_time timestamp with time zone,
    current_duration integer DEFAULT 0,
    current_cost numeric(12,4) DEFAULT 0.0000,
    rate_per_minute numeric(10,6),
    connection_id character varying(100),
    server character varying(100),
    client_id integer,
    last_updated timestamp with time zone DEFAULT now(),
    status character varying(20) DEFAULT 'active'::character varying
);


--
-- Name: active_calls_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.active_calls_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: active_calls_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.active_calls_id_seq OWNED BY public.active_calls.id;


--
-- Name: audit_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs (
    id bigint NOT NULL,
    user_id integer,
    username character varying(100) NOT NULL,
    action character varying(100) NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id character varying(100),
    details jsonb,
    ip_address character varying(45),
    user_agent text,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: audit_logs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.audit_logs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: audit_logs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.audit_logs_id_seq OWNED BY public.audit_logs.id;


--
-- Name: balance_reservations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.balance_reservations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    account_id integer NOT NULL,
    call_uuid character varying(100) NOT NULL,
    reserved_amount numeric(12,4) DEFAULT 0.0000 NOT NULL,
    consumed_amount numeric(12,4) DEFAULT 0.0000 NOT NULL,
    released_amount numeric(12,4) DEFAULT 0.0000 NOT NULL,
    status character varying(20) DEFAULT 'active'::character varying NOT NULL,
    type character varying(20) DEFAULT 'initial'::character varying NOT NULL,
    destination_prefix character varying(20),
    rate_per_minute numeric(10,6),
    reserved_minutes integer,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    consumed_at timestamp with time zone,
    released_at timestamp with time zone,
    created_by character varying(100) DEFAULT 'system'::character varying,
    updated_by character varying(100) DEFAULT 'system'::character varying
);


--
-- Name: balance_transactions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.balance_transactions (
    id bigint NOT NULL,
    account_id integer NOT NULL,
    amount numeric(12,4) NOT NULL,
    previous_balance numeric(12,4) NOT NULL,
    new_balance numeric(12,4) NOT NULL,
    type character varying(20) NOT NULL,
    reason text,
    call_uuid character varying(100),
    reservation_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    created_by character varying(100) DEFAULT 'system'::character varying
);


--
-- Name: balance_transactions_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.balance_transactions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: balance_transactions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.balance_transactions_id_seq OWNED BY public.balance_transactions.id;


--
-- Name: cdrs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.cdrs (
    id bigint NOT NULL,
    call_uuid character varying(100) NOT NULL,
    account_id integer,
    caller_number character varying(50) NOT NULL,
    called_number character varying(50) NOT NULL,
    destination_prefix character varying(20),
    start_time timestamp with time zone NOT NULL,
    answer_time timestamp with time zone,
    end_time timestamp with time zone NOT NULL,
    duration integer DEFAULT 0 NOT NULL,
    billsec integer DEFAULT 0 NOT NULL,
    rate_per_minute numeric(10,6),
    cost numeric(12,4) DEFAULT 0.0000,
    hangup_cause character varying(50),
    direction character varying(20),
    freeswitch_server_id character varying(100),
    reservation_id uuid,
    created_at timestamp with time zone DEFAULT now(),
    processed_at timestamp with time zone,
    rate_id integer
);


--
-- Name: cdrs_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.cdrs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: cdrs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.cdrs_id_seq OWNED BY public.cdrs.id;


--
-- Name: freeswitch_allowed_ips; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.freeswitch_allowed_ips (
    id integer NOT NULL,
    ip_address inet NOT NULL,
    description character varying(255),
    enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE freeswitch_allowed_ips; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.freeswitch_allowed_ips IS 'IPs autorizadas para consultar el endpoint mod_xml_curl';


--
-- Name: COLUMN freeswitch_allowed_ips.ip_address; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.freeswitch_allowed_ips.ip_address IS 'Dirección IP del servidor FreeSWITCH';


--
-- Name: freeswitch_allowed_ips_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.freeswitch_allowed_ips_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: freeswitch_allowed_ips_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.freeswitch_allowed_ips_id_seq OWNED BY public.freeswitch_allowed_ips.id;


--
-- Name: internal_routes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.internal_routes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    route_type character varying(30) NOT NULL,
    source_endpoint_id uuid,
    dest_endpoint_id uuid,
    bypass_media boolean DEFAULT true NOT NULL,
    inherit_codec boolean DEFAULT true NOT NULL,
    enable_100rel boolean DEFAULT true NOT NULL,
    call_timeout integer DEFAULT 60 NOT NULL,
    prefix_pattern character varying(50) DEFAULT '.*'::character varying,
    sync_status character varying(20) DEFAULT 'pending'::character varying NOT NULL,
    sync_error text,
    last_sync timestamp with time zone,
    priority integer DEFAULT 100 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT internal_routes_route_type_check CHECK (((route_type)::text = ANY ((ARRAY['fs_to_kamailio'::character varying, 'kamailio_to_fs'::character varying])::text[]))),
    CONSTRAINT internal_routes_sync_status_check CHECK (((sync_status)::text = ANY ((ARRAY['pending'::character varying, 'synced'::character varying, 'error'::character varying])::text[])))
);


--
-- Name: plans; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plans (
    id integer NOT NULL,
    plan_name character varying(100) NOT NULL,
    plan_code character varying(50) NOT NULL,
    account_type character varying(20) NOT NULL,
    initial_balance numeric(12,4) DEFAULT 0.0000 NOT NULL,
    credit_limit numeric(12,4) DEFAULT 0.0000 NOT NULL,
    max_concurrent_calls integer DEFAULT 5 NOT NULL,
    description text,
    enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    created_by character varying(100) DEFAULT 'system'::character varying,
    CONSTRAINT plans_account_type_check CHECK (((account_type)::text = ANY (ARRAY[('PREPAID'::character varying)::text, ('POSTPAID'::character varying)::text])))
);


--
-- Name: TABLE plans; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.plans IS 'Planes predefinidos para creación rápida de cuentas';


--
-- Name: COLUMN plans.plan_code; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.plans.plan_code IS 'Código único del plan (ej: PRE-BAS, POST-500)';


--
-- Name: COLUMN plans.initial_balance; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.plans.initial_balance IS 'Saldo inicial que se otorga al crear la cuenta';


--
-- Name: COLUMN plans.credit_limit; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.plans.credit_limit IS 'Límite de crédito para cuentas postpago';


--
-- Name: plans_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.plans_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: plans_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.plans_id_seq OWNED BY public.plans.id;


--
-- Name: prefijos; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.prefijos (
    id integer NOT NULL,
    prefix character varying(20) NOT NULL,
    zone_id integer,
    description text,
    enabled boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: prefijos_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.prefijos_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: prefijos_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.prefijos_id_seq OWNED BY public.prefijos.id;


--
-- Name: rate_cards; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.rate_cards (
    id integer NOT NULL,
    rate_name character varying(200) NOT NULL,
    destination_prefix character varying(20) NOT NULL,
    destination_name character varying(200),
    rate_per_minute numeric(10,6) NOT NULL,
    billing_increment integer DEFAULT 6 NOT NULL,
    initial_increment_seconds integer DEFAULT 6 NOT NULL,
    connection_fee numeric(10,6) DEFAULT 0.0,
    priority integer DEFAULT 100 NOT NULL,
    effective_start timestamp with time zone DEFAULT now(),
    effective_end timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    created_by character varying(100) DEFAULT 'system'::character varying
);


--
-- Name: rate_cards_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.rate_cards_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: rate_cards_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.rate_cards_id_seq OWNED BY public.rate_cards.id;


--
-- Name: routing_inbound_routes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.routing_inbound_routes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    did_pattern character varying(100) NOT NULL,
    source_ip_pattern character varying(100),
    priority integer DEFAULT 100,
    destination_host character varying(255) NOT NULL,
    destination_port integer DEFAULT 5060,
    destination_profile character varying(20) DEFAULT 'internal'::character varying,
    call_timeout integer DEFAULT 120,
    inherit_codec boolean DEFAULT true,
    ignore_early_media boolean DEFAULT false,
    bypass_media boolean DEFAULT false,
    failover_destinations jsonb DEFAULT '[]'::jsonb,
    enabled boolean DEFAULT true,
    freeswitch_extension_id character varying(100),
    sync_status character varying(20) DEFAULT 'pending'::character varying,
    sync_error text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    strip_digits integer DEFAULT 0,
    prefix_to_add character varying(20) DEFAULT ''::character varying,
    destination_trunk_id uuid,
    send_early_media boolean DEFAULT false NOT NULL,
    CONSTRAINT routing_inbound_routes_destination_profile_check CHECK (((destination_profile)::text = ANY ((ARRAY['internal'::character varying, 'external'::character varying])::text[])))
);


--
-- Name: TABLE routing_inbound_routes; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.routing_inbound_routes IS 'Rutas entrantes (sincroniza con FreeSWITCH to-kamailio.xml)';


--
-- Name: COLUMN routing_inbound_routes.failover_destinations; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_inbound_routes.failover_destinations IS 'Array JSON de destinos secundarios [{host, port}]';


--
-- Name: COLUMN routing_inbound_routes.strip_digits; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_inbound_routes.strip_digits IS 'Number of digits to strip from the beginning of destination_number before bridge';


--
-- Name: COLUMN routing_inbound_routes.prefix_to_add; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_inbound_routes.prefix_to_add IS 'Prefix to add to destination_number after stripping digits';


--
-- Name: COLUMN routing_inbound_routes.destination_trunk_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_inbound_routes.destination_trunk_id IS 'If set, route via this trunk gateway instead of destination_host:port';


--
-- Name: COLUMN routing_inbound_routes.send_early_media; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_inbound_routes.send_early_media IS 'When true, sends 183 Session Progress with ringback tone before bridging';


--
-- Name: routing_outbound_routes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.routing_outbound_routes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    prefix_pattern character varying(50) NOT NULL,
    priority integer DEFAULT 100,
    trunk_group_id uuid,
    trunk_id uuid,
    time_schedule character varying(255),
    time_schedule_enabled boolean DEFAULT false,
    enabled boolean DEFAULT true,
    kamailio_ruleid integer,
    sync_status character varying(20) DEFAULT 'pending'::character varying,
    sync_error text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT routing_outbound_routes_destination_check CHECK ((((trunk_group_id IS NOT NULL) AND (trunk_id IS NULL)) OR ((trunk_group_id IS NULL) AND (trunk_id IS NOT NULL))))
);


--
-- Name: TABLE routing_outbound_routes; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.routing_outbound_routes IS 'Rutas salientes (sincroniza con Kamailio dr_rules)';


--
-- Name: COLUMN routing_outbound_routes.time_schedule; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_outbound_routes.time_schedule IS 'Formato timerec de Kamailio (ej: * * * * 1-5 09:00-18:00)';


--
-- Name: routing_sip_status_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.routing_sip_status_log (
    id bigint NOT NULL,
    trunk_id uuid NOT NULL,
    check_timestamp timestamp with time zone DEFAULT now(),
    check_source character varying(20) NOT NULL,
    status character varying(20) NOT NULL,
    response_code integer,
    latency_ms integer,
    request_sent text,
    response_received text,
    error_message text,
    peer_user_agent character varying(255),
    peer_allow_methods character varying(255),
    CONSTRAINT routing_sip_status_log_check_source_check CHECK (((check_source)::text = ANY ((ARRAY['freeswitch'::character varying, 'kamailio'::character varying])::text[]))),
    CONSTRAINT routing_sip_status_log_status_check CHECK (((status)::text = ANY ((ARRAY['success'::character varying, 'timeout'::character varying, 'error'::character varying, 'rejected'::character varying])::text[])))
);


--
-- Name: TABLE routing_sip_status_log; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.routing_sip_status_log IS 'Historial de verificaciones OPTIONS para cada troncal';


--
-- Name: COLUMN routing_sip_status_log.check_source; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_sip_status_log.check_source IS 'Sistema que realizó la verificación: freeswitch o kamailio';


--
-- Name: COLUMN routing_sip_status_log.request_sent; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_sip_status_log.request_sent IS 'Mensaje OPTIONS SIP enviado (para debug)';


--
-- Name: COLUMN routing_sip_status_log.response_received; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_sip_status_log.response_received IS 'Respuesta SIP recibida (para debug)';


--
-- Name: routing_sip_status_log_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.routing_sip_status_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: routing_sip_status_log_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.routing_sip_status_log_id_seq OWNED BY public.routing_sip_status_log.id;


--
-- Name: routing_trunk_group_members; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.routing_trunk_group_members (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    group_id uuid NOT NULL,
    trunk_id uuid NOT NULL,
    priority integer DEFAULT 1,
    weight integer DEFAULT 100,
    max_channels integer,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: TABLE routing_trunk_group_members; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.routing_trunk_group_members IS 'Relación trunk-grupo con prioridad y peso';


--
-- Name: routing_trunk_groups; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.routing_trunk_groups (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    failover_strategy character varying(20) DEFAULT 'sequential'::character varying,
    kamailio_group_id integer,
    sync_status character varying(20) DEFAULT 'pending'::character varying,
    sync_error text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    CONSTRAINT routing_trunk_groups_failover_strategy_check CHECK (((failover_strategy)::text = ANY ((ARRAY['sequential'::character varying, 'round_robin'::character varying, 'weighted'::character varying, 'least_calls'::character varying])::text[])))
);


--
-- Name: TABLE routing_trunk_groups; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.routing_trunk_groups IS 'Grupos de failover (sincroniza con Kamailio dr_gw_lists)';


--
-- Name: COLUMN routing_trunk_groups.failover_strategy; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunk_groups.failover_strategy IS 'sequential=en orden, round_robin=rotativo, weighted=por peso, least_calls=menos ocupado';


--
-- Name: routing_trunks; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.routing_trunks (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    host character varying(255) NOT NULL,
    port integer DEFAULT 5060,
    transport character varying(10) DEFAULT 'udp'::character varying,
    auth_username character varying(100),
    auth_password_encrypted bytea,
    strip_digits integer DEFAULT 0,
    prefix_to_add character varying(20) DEFAULT ''::character varying,
    enabled boolean DEFAULT true,
    kamailio_gwid integer,
    sync_status character varying(20) DEFAULT 'pending'::character varying,
    sync_error text,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    trunk_type character varying(10) DEFAULT 'public'::character varying,
    freeswitch_gateway_name character varying(100),
    sip_status character varying(20) DEFAULT 'unknown'::character varying,
    sip_status_message text,
    last_options_check timestamp with time zone,
    last_options_latency_ms integer,
    last_options_response_code integer,
    auth_password_nonce bytea,
    CONSTRAINT routing_trunks_sip_status_check CHECK (((sip_status)::text = ANY ((ARRAY['unknown'::character varying, 'reachable'::character varying, 'unreachable'::character varying, 'checking'::character varying])::text[]))),
    CONSTRAINT routing_trunks_transport_check CHECK (((transport)::text = ANY ((ARRAY['udp'::character varying, 'tcp'::character varying, 'tls'::character varying])::text[]))),
    CONSTRAINT routing_trunks_trunk_type_check CHECK (((trunk_type)::text = ANY ((ARRAY['private'::character varying, 'public'::character varying])::text[])))
);


--
-- Name: TABLE routing_trunks; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.routing_trunks IS 'Trunks/carriers unificados (sincroniza con Kamailio dr_gateways)';


--
-- Name: COLUMN routing_trunks.kamailio_gwid; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunks.kamailio_gwid IS 'ID del gateway en Kamailio (dr_gateways.gwid)';


--
-- Name: COLUMN routing_trunks.trunk_type; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunks.trunk_type IS 'private=red interna (FreeSWITCH), public=otros operadores (Kamailio)';


--
-- Name: COLUMN routing_trunks.freeswitch_gateway_name; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunks.freeswitch_gateway_name IS 'Nombre del gateway en FreeSWITCH (solo para trunk_type=private)';


--
-- Name: COLUMN routing_trunks.sip_status; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunks.sip_status IS 'Estado de conexión SIP: unknown, reachable, unreachable, checking';


--
-- Name: COLUMN routing_trunks.sip_status_message; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunks.sip_status_message IS 'Mensaje descriptivo del estado SIP actual';


--
-- Name: COLUMN routing_trunks.last_options_check; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunks.last_options_check IS 'Timestamp de la última verificación OPTIONS';


--
-- Name: COLUMN routing_trunks.last_options_latency_ms; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunks.last_options_latency_ms IS 'Latencia en ms de la última respuesta OPTIONS';


--
-- Name: COLUMN routing_trunks.last_options_response_code; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.routing_trunks.last_options_response_code IS 'Código SIP de la última respuesta OPTIONS (200, 403, etc)';


--
-- Name: sip_devices; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.sip_devices (
    id integer NOT NULL,
    account_id integer NOT NULL,
    sip_username character varying(64) NOT NULL,
    sip_domain character varying(255) DEFAULT 'apolo.local'::character varying NOT NULL,
    password_encrypted bytea NOT NULL,
    password_nonce bytea NOT NULL,
    a1_hash character varying(32) NOT NULL,
    display_name character varying(100),
    description text,
    context character varying(50) DEFAULT 'from-pbx'::character varying NOT NULL,
    accountcode character varying(50),
    codecs character varying(255) DEFAULT 'PCMU,PCMA,G729,opus'::character varying,
    max_registrations integer DEFAULT 3 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: TABLE sip_devices; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.sip_devices IS 'Dispositivos SIP registrados en el sistema, autenticados via mod_xml_curl';


--
-- Name: COLUMN sip_devices.sip_username; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.sip_username IS 'Usuario SIP (extensión o nombre de usuario)';


--
-- Name: COLUMN sip_devices.sip_domain; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.sip_domain IS 'Dominio SIP (realm para autenticación)';


--
-- Name: COLUMN sip_devices.password_encrypted; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.password_encrypted IS 'Contraseña encriptada con AES-256-GCM';


--
-- Name: COLUMN sip_devices.password_nonce; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.password_nonce IS 'Nonce usado para encriptación AES-256-GCM';


--
-- Name: COLUMN sip_devices.a1_hash; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.a1_hash IS 'Hash MD5(username:realm:password) para autenticación digest';


--
-- Name: COLUMN sip_devices.context; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.context IS 'Contexto de dialplan FreeSWITCH (from-pbx, from-internal, etc.)';


--
-- Name: COLUMN sip_devices.accountcode; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.accountcode IS 'Código de cuenta para vincular CDRs con facturación';


--
-- Name: COLUMN sip_devices.codecs; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.codecs IS 'Lista de codecs permitidos separados por coma';


--
-- Name: COLUMN sip_devices.max_registrations; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.sip_devices.max_registrations IS 'Número máximo de registros simultáneos permitidos';


--
-- Name: sip_devices_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.sip_devices_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: sip_devices_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.sip_devices_id_seq OWNED BY public.sip_devices.id;


--
-- Name: system_endpoints; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_endpoints (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    endpoint_type character varying(30) NOT NULL,
    description text,
    ip_address character varying(45) NOT NULL,
    port integer NOT NULL,
    transport character varying(10) DEFAULT 'udp'::character varying NOT NULL,
    fs_profile character varying(50),
    fs_context character varying(50),
    kam_gwid integer,
    kam_gw_type integer DEFAULT 9,
    enabled boolean DEFAULT true NOT NULL,
    is_primary boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT system_endpoints_endpoint_type_check CHECK (((endpoint_type)::text = ANY ((ARRAY['freeswitch'::character varying, 'kamailio'::character varying])::text[]))),
    CONSTRAINT system_endpoints_transport_check CHECK (((transport)::text = ANY ((ARRAY['udp'::character varying, 'tcp'::character varying, 'tls'::character varying])::text[])))
);


--
-- Name: system_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.system_settings (
    key character varying(100) NOT NULL,
    value text NOT NULL,
    description text,
    updated_at timestamp with time zone DEFAULT now(),
    updated_by character varying(100)
);


--
-- Name: tarifas; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.tarifas (
    id integer NOT NULL,
    zone_id integer,
    rate_per_minute numeric(10,6) NOT NULL,
    billing_increment integer DEFAULT 6,
    connection_fee numeric(10,6) DEFAULT 0.0,
    effective_start timestamp with time zone DEFAULT now(),
    effective_end timestamp with time zone,
    enabled boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: tarifas_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.tarifas_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: tarifas_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.tarifas_id_seq OWNED BY public.tarifas.id;


--
-- Name: usuarios; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.usuarios (
    id integer NOT NULL,
    username character varying(100) NOT NULL,
    password text NOT NULL,
    nombre character varying(100),
    apellido character varying(100),
    email character varying(255),
    role character varying(20) DEFAULT 'operator'::character varying NOT NULL,
    activo boolean DEFAULT true NOT NULL,
    ultimo_login timestamp with time zone,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: usuarios_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.usuarios_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: usuarios_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.usuarios_id_seq OWNED BY public.usuarios.id;


--
-- Name: zonas; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.zonas (
    id integer NOT NULL,
    zone_name character varying(200) NOT NULL,
    zone_code character varying(50),
    zone_type character varying(50),
    network_type character varying(50),
    region_name character varying(200),
    description text,
    enabled boolean DEFAULT true,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: zonas_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.zonas_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: zonas_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.zonas_id_seq OWNED BY public.zonas.id;


--
-- Name: accounts id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.accounts ALTER COLUMN id SET DEFAULT nextval('public.accounts_id_seq'::regclass);


--
-- Name: active_calls id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.active_calls ALTER COLUMN id SET DEFAULT nextval('public.active_calls_id_seq'::regclass);


--
-- Name: audit_logs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs ALTER COLUMN id SET DEFAULT nextval('public.audit_logs_id_seq'::regclass);


--
-- Name: balance_transactions id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.balance_transactions ALTER COLUMN id SET DEFAULT nextval('public.balance_transactions_id_seq'::regclass);


--
-- Name: cdrs id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cdrs ALTER COLUMN id SET DEFAULT nextval('public.cdrs_id_seq'::regclass);


--
-- Name: freeswitch_allowed_ips id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.freeswitch_allowed_ips ALTER COLUMN id SET DEFAULT nextval('public.freeswitch_allowed_ips_id_seq'::regclass);


--
-- Name: plans id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plans ALTER COLUMN id SET DEFAULT nextval('public.plans_id_seq'::regclass);


--
-- Name: prefijos id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.prefijos ALTER COLUMN id SET DEFAULT nextval('public.prefijos_id_seq'::regclass);


--
-- Name: rate_cards id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rate_cards ALTER COLUMN id SET DEFAULT nextval('public.rate_cards_id_seq'::regclass);


--
-- Name: routing_sip_status_log id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_sip_status_log ALTER COLUMN id SET DEFAULT nextval('public.routing_sip_status_log_id_seq'::regclass);


--
-- Name: sip_devices id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sip_devices ALTER COLUMN id SET DEFAULT nextval('public.sip_devices_id_seq'::regclass);


--
-- Name: tarifas id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tarifas ALTER COLUMN id SET DEFAULT nextval('public.tarifas_id_seq'::regclass);


--
-- Name: usuarios id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.usuarios ALTER COLUMN id SET DEFAULT nextval('public.usuarios_id_seq'::regclass);


--
-- Name: zonas id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zonas ALTER COLUMN id SET DEFAULT nextval('public.zonas_id_seq'::regclass);


--
-- Name: accounts accounts_account_number_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_account_number_key UNIQUE (account_number);


--
-- Name: accounts accounts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_pkey PRIMARY KEY (id);


--
-- Name: active_calls active_calls_call_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.active_calls
    ADD CONSTRAINT active_calls_call_id_key UNIQUE (call_id);


--
-- Name: active_calls active_calls_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.active_calls
    ADD CONSTRAINT active_calls_pkey PRIMARY KEY (id);


--
-- Name: audit_logs audit_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_pkey PRIMARY KEY (id);


--
-- Name: balance_reservations balance_reservations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.balance_reservations
    ADD CONSTRAINT balance_reservations_pkey PRIMARY KEY (id);


--
-- Name: balance_transactions balance_transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.balance_transactions
    ADD CONSTRAINT balance_transactions_pkey PRIMARY KEY (id);


--
-- Name: cdrs cdrs_call_uuid_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cdrs
    ADD CONSTRAINT cdrs_call_uuid_key UNIQUE (call_uuid);


--
-- Name: cdrs cdrs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cdrs
    ADD CONSTRAINT cdrs_pkey PRIMARY KEY (id);


--
-- Name: freeswitch_allowed_ips freeswitch_allowed_ips_ip_address_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.freeswitch_allowed_ips
    ADD CONSTRAINT freeswitch_allowed_ips_ip_address_key UNIQUE (ip_address);


--
-- Name: freeswitch_allowed_ips freeswitch_allowed_ips_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.freeswitch_allowed_ips
    ADD CONSTRAINT freeswitch_allowed_ips_pkey PRIMARY KEY (id);


--
-- Name: internal_routes internal_routes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.internal_routes
    ADD CONSTRAINT internal_routes_pkey PRIMARY KEY (id);


--
-- Name: plans plans_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plans
    ADD CONSTRAINT plans_pkey PRIMARY KEY (id);


--
-- Name: plans plans_plan_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plans
    ADD CONSTRAINT plans_plan_code_key UNIQUE (plan_code);


--
-- Name: prefijos prefijos_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.prefijos
    ADD CONSTRAINT prefijos_pkey PRIMARY KEY (id);


--
-- Name: rate_cards rate_cards_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.rate_cards
    ADD CONSTRAINT rate_cards_pkey PRIMARY KEY (id);


--
-- Name: routing_inbound_routes routing_inbound_routes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_inbound_routes
    ADD CONSTRAINT routing_inbound_routes_pkey PRIMARY KEY (id);


--
-- Name: routing_outbound_routes routing_outbound_routes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_outbound_routes
    ADD CONSTRAINT routing_outbound_routes_pkey PRIMARY KEY (id);


--
-- Name: routing_sip_status_log routing_sip_status_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_sip_status_log
    ADD CONSTRAINT routing_sip_status_log_pkey PRIMARY KEY (id);


--
-- Name: routing_trunk_group_members routing_trunk_group_members_group_id_trunk_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_trunk_group_members
    ADD CONSTRAINT routing_trunk_group_members_group_id_trunk_id_key UNIQUE (group_id, trunk_id);


--
-- Name: routing_trunk_group_members routing_trunk_group_members_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_trunk_group_members
    ADD CONSTRAINT routing_trunk_group_members_pkey PRIMARY KEY (id);


--
-- Name: routing_trunk_groups routing_trunk_groups_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_trunk_groups
    ADD CONSTRAINT routing_trunk_groups_pkey PRIMARY KEY (id);


--
-- Name: routing_trunks routing_trunks_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_trunks
    ADD CONSTRAINT routing_trunks_pkey PRIMARY KEY (id);


--
-- Name: sip_devices sip_devices_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sip_devices
    ADD CONSTRAINT sip_devices_pkey PRIMARY KEY (id);


--
-- Name: system_endpoints system_endpoints_endpoint_type_ip_address_port_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_endpoints
    ADD CONSTRAINT system_endpoints_endpoint_type_ip_address_port_key UNIQUE (endpoint_type, ip_address, port);


--
-- Name: system_endpoints system_endpoints_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_endpoints
    ADD CONSTRAINT system_endpoints_pkey PRIMARY KEY (id);


--
-- Name: system_settings system_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.system_settings
    ADD CONSTRAINT system_settings_pkey PRIMARY KEY (key);


--
-- Name: tarifas tarifas_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tarifas
    ADD CONSTRAINT tarifas_pkey PRIMARY KEY (id);


--
-- Name: sip_devices unique_sip_user_domain; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sip_devices
    ADD CONSTRAINT unique_sip_user_domain UNIQUE (sip_username, sip_domain);


--
-- Name: usuarios usuarios_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.usuarios
    ADD CONSTRAINT usuarios_pkey PRIMARY KEY (id);


--
-- Name: usuarios usuarios_username_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.usuarios
    ADD CONSTRAINT usuarios_username_key UNIQUE (username);


--
-- Name: zonas zonas_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zonas
    ADD CONSTRAINT zonas_pkey PRIMARY KEY (id);


--
-- Name: idx_accounts_number; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_accounts_number ON public.accounts USING btree (account_number);


--
-- Name: idx_accounts_plan; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_accounts_plan ON public.accounts USING btree (plan_id);


--
-- Name: idx_accounts_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_accounts_status ON public.accounts USING btree (status);


--
-- Name: idx_accounts_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_accounts_type ON public.accounts USING btree (account_type);


--
-- Name: idx_active_calls_call_id; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_active_calls_call_id ON public.active_calls USING btree (call_id);


--
-- Name: idx_active_calls_client; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_active_calls_client ON public.active_calls USING btree (client_id);


--
-- Name: idx_active_calls_start; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_active_calls_start ON public.active_calls USING btree (start_time);


--
-- Name: idx_audit_logs_action; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_action ON public.audit_logs USING btree (action);


--
-- Name: idx_audit_logs_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_created ON public.audit_logs USING btree (created_at DESC);


--
-- Name: idx_audit_logs_entity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_entity ON public.audit_logs USING btree (entity_type, entity_id);


--
-- Name: idx_audit_logs_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_user ON public.audit_logs USING btree (user_id);


--
-- Name: idx_audit_logs_username; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_username ON public.audit_logs USING btree (username);


--
-- Name: idx_cdr_account; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdr_account ON public.cdrs USING btree (account_id);


--
-- Name: idx_cdr_account_start; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdr_account_start ON public.cdrs USING btree (account_id, start_time);


--
-- Name: idx_cdr_callee; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdr_callee ON public.cdrs USING btree (called_number);


--
-- Name: idx_cdr_caller; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdr_caller ON public.cdrs USING btree (caller_number);


--
-- Name: idx_cdr_reservation; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdr_reservation ON public.cdrs USING btree (reservation_id);


--
-- Name: idx_cdr_start_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdr_start_time ON public.cdrs USING btree (start_time);


--
-- Name: idx_cdr_uuid; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdr_uuid ON public.cdrs USING btree (call_uuid);


--
-- Name: idx_cdrs_account_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdrs_account_id ON public.cdrs USING btree (account_id);


--
-- Name: idx_cdrs_caller; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdrs_caller ON public.cdrs USING btree (caller_number);


--
-- Name: idx_cdrs_start_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdrs_start_time ON public.cdrs USING btree (start_time);


--
-- Name: idx_cdrs_uuid; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_cdrs_uuid ON public.cdrs USING btree (call_uuid);


--
-- Name: idx_freeswitch_ips_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_freeswitch_ips_enabled ON public.freeswitch_allowed_ips USING btree (enabled) WHERE (enabled = true);


--
-- Name: idx_internal_routes_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_internal_routes_enabled ON public.internal_routes USING btree (enabled);


--
-- Name: idx_internal_routes_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_internal_routes_type ON public.internal_routes USING btree (route_type);


--
-- Name: idx_plans_code; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_plans_code ON public.plans USING btree (plan_code);


--
-- Name: idx_plans_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_plans_enabled ON public.plans USING btree (enabled);


--
-- Name: idx_plans_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_plans_type ON public.plans USING btree (account_type);


--
-- Name: idx_prefijos_prefix; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_prefijos_prefix ON public.prefijos USING btree (prefix);


--
-- Name: idx_prefijos_zone; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_prefijos_zone ON public.prefijos USING btree (zone_id);


--
-- Name: idx_rate_cards_dates; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rate_cards_dates ON public.rate_cards USING btree (effective_start, effective_end);


--
-- Name: idx_rate_cards_prefix; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rate_cards_prefix ON public.rate_cards USING btree (destination_prefix);


--
-- Name: idx_rate_cards_prefix_priority; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rate_cards_prefix_priority ON public.rate_cards USING btree (destination_prefix, priority DESC);


--
-- Name: idx_rate_cards_priority; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_rate_cards_priority ON public.rate_cards USING btree (priority DESC);


--
-- Name: idx_reservations_account; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reservations_account ON public.balance_reservations USING btree (account_id);


--
-- Name: idx_reservations_account_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reservations_account_status ON public.balance_reservations USING btree (account_id, status);


--
-- Name: idx_reservations_call; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reservations_call ON public.balance_reservations USING btree (call_uuid);


--
-- Name: idx_reservations_expires; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reservations_expires ON public.balance_reservations USING btree (expires_at);


--
-- Name: idx_reservations_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_reservations_status ON public.balance_reservations USING btree (status);


--
-- Name: idx_routing_inbound_routes_did; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_inbound_routes_did ON public.routing_inbound_routes USING btree (did_pattern);


--
-- Name: idx_routing_inbound_routes_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_inbound_routes_enabled ON public.routing_inbound_routes USING btree (enabled);


--
-- Name: idx_routing_inbound_routes_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_inbound_routes_name ON public.routing_inbound_routes USING btree (name);


--
-- Name: idx_routing_inbound_routes_priority; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_inbound_routes_priority ON public.routing_inbound_routes USING btree (priority);


--
-- Name: idx_routing_inbound_routes_sync_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_inbound_routes_sync_status ON public.routing_inbound_routes USING btree (sync_status);


--
-- Name: idx_routing_outbound_routes_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_outbound_routes_enabled ON public.routing_outbound_routes USING btree (enabled);


--
-- Name: idx_routing_outbound_routes_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_outbound_routes_name ON public.routing_outbound_routes USING btree (name);


--
-- Name: idx_routing_outbound_routes_prefix; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_outbound_routes_prefix ON public.routing_outbound_routes USING btree (prefix_pattern);


--
-- Name: idx_routing_outbound_routes_priority; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_outbound_routes_priority ON public.routing_outbound_routes USING btree (priority);


--
-- Name: idx_routing_outbound_routes_sync_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_outbound_routes_sync_status ON public.routing_outbound_routes USING btree (sync_status);


--
-- Name: idx_routing_sip_status_log_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_sip_status_log_status ON public.routing_sip_status_log USING btree (status);


--
-- Name: idx_routing_sip_status_log_timestamp; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_sip_status_log_timestamp ON public.routing_sip_status_log USING btree (check_timestamp DESC);


--
-- Name: idx_routing_sip_status_log_trunk; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_sip_status_log_trunk ON public.routing_sip_status_log USING btree (trunk_id);


--
-- Name: idx_routing_trunk_group_members_group; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunk_group_members_group ON public.routing_trunk_group_members USING btree (group_id);


--
-- Name: idx_routing_trunk_group_members_priority; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunk_group_members_priority ON public.routing_trunk_group_members USING btree (priority);


--
-- Name: idx_routing_trunk_group_members_trunk; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunk_group_members_trunk ON public.routing_trunk_group_members USING btree (trunk_id);


--
-- Name: idx_routing_trunk_groups_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunk_groups_name ON public.routing_trunk_groups USING btree (name);


--
-- Name: idx_routing_trunk_groups_sync_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunk_groups_sync_status ON public.routing_trunk_groups USING btree (sync_status);


--
-- Name: idx_routing_trunks_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunks_enabled ON public.routing_trunks USING btree (enabled);


--
-- Name: idx_routing_trunks_kamailio_gwid; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunks_kamailio_gwid ON public.routing_trunks USING btree (kamailio_gwid);


--
-- Name: idx_routing_trunks_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunks_name ON public.routing_trunks USING btree (name);


--
-- Name: idx_routing_trunks_sip_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunks_sip_status ON public.routing_trunks USING btree (sip_status);


--
-- Name: idx_routing_trunks_sync_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunks_sync_status ON public.routing_trunks USING btree (sync_status);


--
-- Name: idx_routing_trunks_trunk_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_routing_trunks_trunk_type ON public.routing_trunks USING btree (trunk_type);


--
-- Name: idx_sip_devices_account; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sip_devices_account ON public.sip_devices USING btree (account_id);


--
-- Name: idx_sip_devices_accountcode; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sip_devices_accountcode ON public.sip_devices USING btree (accountcode);


--
-- Name: idx_sip_devices_domain; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sip_devices_domain ON public.sip_devices USING btree (sip_domain);


--
-- Name: idx_sip_devices_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sip_devices_enabled ON public.sip_devices USING btree (enabled) WHERE (enabled = true);


--
-- Name: idx_sip_devices_username; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_sip_devices_username ON public.sip_devices USING btree (sip_username);


--
-- Name: idx_system_endpoints_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_endpoints_type ON public.system_endpoints USING btree (endpoint_type);


--
-- Name: idx_tarifas_dates; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_tarifas_dates ON public.tarifas USING btree (effective_start, effective_end);


--
-- Name: idx_tarifas_zone; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_tarifas_zone ON public.tarifas USING btree (zone_id);


--
-- Name: idx_transactions_account; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_transactions_account ON public.balance_transactions USING btree (account_id);


--
-- Name: idx_transactions_call; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_transactions_call ON public.balance_transactions USING btree (call_uuid);


--
-- Name: idx_transactions_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_transactions_created ON public.balance_transactions USING btree (created_at);


--
-- Name: idx_transactions_reservation; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_transactions_reservation ON public.balance_transactions USING btree (reservation_id);


--
-- Name: idx_transactions_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_transactions_type ON public.balance_transactions USING btree (type);


--
-- Name: idx_usuarios_email; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_usuarios_email ON public.usuarios USING btree (email);


--
-- Name: idx_usuarios_username; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_usuarios_username ON public.usuarios USING btree (username);


--
-- Name: ix_rate_cards_destination_prefix; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_rate_cards_destination_prefix ON public.rate_cards USING btree (destination_prefix);


--
-- Name: ix_rate_cards_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_rate_cards_id ON public.rate_cards USING btree (id);


--
-- Name: routing_inbound_routes tr_routing_inbound_routes_updated; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER tr_routing_inbound_routes_updated BEFORE UPDATE ON public.routing_inbound_routes FOR EACH ROW EXECUTE FUNCTION public.update_routing_updated_at();


--
-- Name: routing_outbound_routes tr_routing_outbound_routes_updated; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER tr_routing_outbound_routes_updated BEFORE UPDATE ON public.routing_outbound_routes FOR EACH ROW EXECUTE FUNCTION public.update_routing_updated_at();


--
-- Name: routing_trunk_groups tr_routing_trunk_groups_updated; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER tr_routing_trunk_groups_updated BEFORE UPDATE ON public.routing_trunk_groups FOR EACH ROW EXECUTE FUNCTION public.update_routing_updated_at();


--
-- Name: routing_trunks tr_routing_trunks_updated; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER tr_routing_trunks_updated BEFORE UPDATE ON public.routing_trunks FOR EACH ROW EXECUTE FUNCTION public.update_routing_updated_at();


--
-- Name: freeswitch_allowed_ips trigger_freeswitch_ips_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trigger_freeswitch_ips_updated_at BEFORE UPDATE ON public.freeswitch_allowed_ips FOR EACH ROW EXECUTE FUNCTION public.update_sip_devices_updated_at();


--
-- Name: sip_devices trigger_sip_devices_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trigger_sip_devices_updated_at BEFORE UPDATE ON public.sip_devices FOR EACH ROW EXECUTE FUNCTION public.update_sip_devices_updated_at();


--
-- Name: accounts accounts_plan_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_plan_id_fkey FOREIGN KEY (plan_id) REFERENCES public.plans(id) ON DELETE SET NULL;


--
-- Name: audit_logs audit_logs_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.usuarios(id) ON DELETE SET NULL;


--
-- Name: balance_reservations balance_reservations_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.balance_reservations
    ADD CONSTRAINT balance_reservations_account_id_fkey FOREIGN KEY (account_id) REFERENCES public.accounts(id) ON DELETE CASCADE;


--
-- Name: balance_transactions balance_transactions_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.balance_transactions
    ADD CONSTRAINT balance_transactions_account_id_fkey FOREIGN KEY (account_id) REFERENCES public.accounts(id) ON DELETE CASCADE;


--
-- Name: balance_transactions balance_transactions_reservation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.balance_transactions
    ADD CONSTRAINT balance_transactions_reservation_id_fkey FOREIGN KEY (reservation_id) REFERENCES public.balance_reservations(id) ON DELETE SET NULL;


--
-- Name: cdrs cdrs_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cdrs
    ADD CONSTRAINT cdrs_account_id_fkey FOREIGN KEY (account_id) REFERENCES public.accounts(id) ON DELETE SET NULL;


--
-- Name: cdrs cdrs_reservation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.cdrs
    ADD CONSTRAINT cdrs_reservation_id_fkey FOREIGN KEY (reservation_id) REFERENCES public.balance_reservations(id) ON DELETE SET NULL;


--
-- Name: internal_routes internal_routes_dest_endpoint_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.internal_routes
    ADD CONSTRAINT internal_routes_dest_endpoint_id_fkey FOREIGN KEY (dest_endpoint_id) REFERENCES public.system_endpoints(id) ON DELETE CASCADE;


--
-- Name: internal_routes internal_routes_source_endpoint_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.internal_routes
    ADD CONSTRAINT internal_routes_source_endpoint_id_fkey FOREIGN KEY (source_endpoint_id) REFERENCES public.system_endpoints(id) ON DELETE CASCADE;


--
-- Name: prefijos prefijos_zone_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.prefijos
    ADD CONSTRAINT prefijos_zone_id_fkey FOREIGN KEY (zone_id) REFERENCES public.zonas(id) ON DELETE SET NULL;


--
-- Name: routing_inbound_routes routing_inbound_routes_destination_trunk_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_inbound_routes
    ADD CONSTRAINT routing_inbound_routes_destination_trunk_id_fkey FOREIGN KEY (destination_trunk_id) REFERENCES public.routing_trunks(id) ON DELETE SET NULL;


--
-- Name: routing_outbound_routes routing_outbound_routes_trunk_group_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_outbound_routes
    ADD CONSTRAINT routing_outbound_routes_trunk_group_id_fkey FOREIGN KEY (trunk_group_id) REFERENCES public.routing_trunk_groups(id) ON DELETE SET NULL;


--
-- Name: routing_outbound_routes routing_outbound_routes_trunk_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_outbound_routes
    ADD CONSTRAINT routing_outbound_routes_trunk_id_fkey FOREIGN KEY (trunk_id) REFERENCES public.routing_trunks(id) ON DELETE SET NULL;


--
-- Name: routing_sip_status_log routing_sip_status_log_trunk_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_sip_status_log
    ADD CONSTRAINT routing_sip_status_log_trunk_id_fkey FOREIGN KEY (trunk_id) REFERENCES public.routing_trunks(id) ON DELETE CASCADE;


--
-- Name: routing_trunk_group_members routing_trunk_group_members_group_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_trunk_group_members
    ADD CONSTRAINT routing_trunk_group_members_group_id_fkey FOREIGN KEY (group_id) REFERENCES public.routing_trunk_groups(id) ON DELETE CASCADE;


--
-- Name: routing_trunk_group_members routing_trunk_group_members_trunk_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.routing_trunk_group_members
    ADD CONSTRAINT routing_trunk_group_members_trunk_id_fkey FOREIGN KEY (trunk_id) REFERENCES public.routing_trunks(id) ON DELETE CASCADE;


--
-- Name: sip_devices sip_devices_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.sip_devices
    ADD CONSTRAINT sip_devices_account_id_fkey FOREIGN KEY (account_id) REFERENCES public.accounts(id) ON DELETE CASCADE;


--
-- Name: tarifas tarifas_zone_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tarifas
    ADD CONSTRAINT tarifas_zone_id_fkey FOREIGN KEY (zone_id) REFERENCES public.zonas(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict VZZ2gHDkXpogb5shkXx0YBn58SzcG4XFsLTEkqwumd27KR3jtZiBCMq2WWC8oCi

