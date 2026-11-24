"""Ephemeral SAP server and client certificates; never installed in Windows."""
import datetime as dt

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import ExtendedKeyUsageOID as Usage, NameOID


def certificate_files(root, hosts):
    now = dt.datetime.now(dt.timezone.utc)
    ca_key = ec.generate_private_key(ec.SECP256R1())
    issuer = name("SAP isolated fixture CA")

    def issue(subject, key):
        return (x509.CertificateBuilder().subject_name(subject).issuer_name(issuer)
                .public_key(key.public_key()).serial_number(x509.random_serial_number())
                .not_valid_before(now - dt.timedelta(minutes=5)).not_valid_after(now + dt.timedelta(hours=1)))

    def write(filename, content):
        path = root / filename
        with path.open("xb") as output:
            output.write(content)
        return path

    ca = (issue(issuer, ca_key).add_extension(x509.BasicConstraints(ca=True, path_length=0), True)
          .sign(ca_key, hashes.SHA256()))
    server_key = ec.generate_private_key(ec.SECP256R1())
    server = (issue(name(hosts[0]), server_key)
              .add_extension(x509.SubjectAlternativeName([x509.DNSName(host) for host in hosts]), False)
              .add_extension(x509.ExtendedKeyUsage([Usage.SERVER_AUTH]), False).sign(ca_key, hashes.SHA256()))
    client_key = ec.generate_private_key(ec.SECP256R1())
    client = (issue(name("SAP synthetic client"), client_key)
              .add_extension(x509.ExtendedKeyUsage([Usage.CLIENT_AUTH]), False).sign(ca_key, hashes.SHA256()))
    pem = serialization.Encoding.PEM
    return (write("fixture-ca.pem", ca.public_bytes(pem)), write("leaf.pem", server.public_bytes(pem)),
            write("leaf-key.pem", key_pem(server_key)), client.public_bytes(pem).decode("ascii"),
            key_pem(client_key).decode("ascii"), client.public_bytes(serialization.Encoding.DER))


def name(text):
    return x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, text)])


def key_pem(key):
    return key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8,
                             serialization.NoEncryption())
