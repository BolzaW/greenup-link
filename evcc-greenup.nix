{ config, pkgs, ... }:

{
  services.evcc = {
    enable = true;
    
    settings = {
      chargers = [
        {
          name = "greenup";
          type = "custom";
          
          status = {
            source = "http";
            uri = "http://127.0.0.1:3000/api/telemetry";
            jq = ".evcc_status";
            timeout = "5s";
          };
          
          power = {
            source = "http";
            uri = "http://127.0.0.1:3000/api/telemetry";
            jq = ".power";
            timeout = "5s";
          };
          
          energy = {
            source = "http";
            uri = "http://127.0.0.1:3000/api/telemetry";
            jq = ".energy";
            timeout = "5s";
          };
          
          enabled = {
            source = "http";
            uri = "http://127.0.0.1:3000/api/telemetry";
            jq = ".evcc_enabled";
            timeout = "5s";
          };
          
          enable = {
            source = "http";
            uri = "http://127.0.0.1:3000/api/evcc/state";
            method = "POST";
            headers = [ "Content-Type: application/json" ];
            body = "{\"enable\": \}";
          };
          
          maxcurrent = {
            source = "script";
            cmd = "${pkgs.curl}/bin/curl -s -X POST http://127.0.0.1:3000/api/current/\";
          };
        }
      ];

      loadpoints = [
        {
          title = "Garage";
          charger = "greenup";
          mode = "pv";
          mincurrent = 7;
          maxcurrent = 32;
        }
      ];

      site = {
        title = "Maison";
        meters = {
          grid = "my_grid_meter";
        };
      };
    };
  };
}
