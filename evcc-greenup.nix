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
            jq = ".iec_state";
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
            jq = ".charge_authorized";
            timeout = "5s";
          };
          
          enable = {
            source = "script";
            cmd = "${pkgs.bash}/bin/sh -c "if [ '\\' = 'true' ]; then ${pkgs.curl}/bin/curl -s -X POST -H 'Content-Type: application/json' -d '{\\"action\\":\\"enable\\"}' http://127.0.0.1:3000/api/charge; else ${pkgs.curl}/bin/curl -s -X POST -H 'Content-Type: application/json' -d '{\\"action\\":\\"disable\\"}' http://127.0.0.1:3000/api/charge; fi"";
          };
          
          maxcurrent = {
            source = "script";
            cmd = "${pkgs.curl}/bin/curl -s -X POST http://127.0.0.1:3000/api/current/\\";
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
