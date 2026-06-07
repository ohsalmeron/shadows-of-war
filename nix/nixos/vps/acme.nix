{ config, ... }:

{
  security.acme = {
    acceptTerms = true;
    defaults.email = config.sow.acmeEmail;
  };
}
