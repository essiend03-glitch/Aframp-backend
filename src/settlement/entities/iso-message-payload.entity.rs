CREATE TABLE iso_message_payloads (
    id UUID PRIMARY KEY,

    message_id VARCHAR(255) UNIQUE NOT NULL,

    message_type VARCHAR(50) NOT NULL,

    sender_bic VARCHAR(32) NOT NULL,

    receiver_bic VARCHAR(32) NOT NULL,

    validation_status VARCHAR(32) NOT NULL,

    processing_state VARCHAR(32) NOT NULL,

    xml_payload TEXT NOT NULL,

    checksum VARCHAR(255),

    created_at TIMESTAMP NOT NULL,

    processed_at TIMESTAMP
);