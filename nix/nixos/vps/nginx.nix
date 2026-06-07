{ ... }:

let
  playCsp =
    ''add_header Content-Security-Policy "frame-ancestors 'self' https://shadowsofwar.io https://www.shadowsofwar.io http://127.0.0.1:* http://localhost:*" always;'';
  ptrRobots = ''add_header X-Robots-Tag "noindex, nofollow" always;'';
in
{
  services.nginx = {
    enable = true;
    recommendedGzipSettings = true;
    recommendedProxySettings = true;
    recommendedOptimisation = true;
    recommendedBrotliSettings = true;

    virtualHosts."shadowsofwar.io" = {
      serverName = "shadowsofwar.io";
      serverAliases = [ "www.shadowsofwar.io" ];
      root = "/var/www/shadowsofwar.io/html";
      enableACME = true;
      forceSSL = true;
      extraConfig = ''
        index index.html;

        gzip_vary on;
        gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript application/wasm image/svg+xml;

        location = /privacy.html {
            return 301 /privacy;
        }

        location = /sow.svg {
            add_header Cache-Control "public, max-age=2592000";
            try_files $uri =404;
        }

        location = /game-embed.js {
            add_header Cache-Control "public, max-age=3600";
            try_files $uri =404;
        }

        location = /robots.txt {
            add_header Cache-Control "public, max-age=86400";
            try_files /robots.txt =404;
        }

        location = /sitemap.xml {
            types { application/xml xml; }
            default_type application/xml;
            charset utf-8;
            add_header Cache-Control "public, max-age=86400";
            try_files /sitemap.xml =404;
        }

        location ~ ^/assets/cdn/leaders/(.+)$ {
            root /var/www/shadowsofwar.io/html;
            try_files /assets/cdn/leaders/$1 =404;
            add_header Access-Control-Allow-Origin "*" always;
            add_header Access-Control-Allow-Methods "GET, OPTIONS" always;
            add_header Cache-Control "public, max-age=2592000";
        }
        location ~ ^/assets/cdn/ui/(.+)$ {
            root /var/www/shadowsofwar.io/html;
            try_files /assets/cdn/ui/$1 =404;
            add_header Access-Control-Allow-Origin "*" always;
            add_header Access-Control-Allow-Methods "GET, OPTIONS" always;
            add_header Cache-Control "public, max-age=2592000";
        }
        location ~ ^/assets/cdn/avatars/(.+)$ {
            root /var/www/shadowsofwar.io/html;
            try_files /assets/cdn/avatars/$1 =404;
            add_header Access-Control-Allow-Origin "*" always;
            add_header Access-Control-Allow-Methods "GET, OPTIONS" always;
            add_header Cache-Control "public, max-age=2592000";
        }

        location ~ ^/assets/streamed/(.+)$ {
            rewrite ^/assets/streamed/(.+)$ /assets/cdn/$1 permanent;
        }
        location ~ ^/assets/static/ui/(.+)$ {
            rewrite ^/assets/static/ui/(.+)$ /assets/cdn/ui/$1 permanent;
        }
        location ~ ^/assets/ui/leaders/(.+)$ {
            rewrite ^/assets/ui/leaders/(.+)$ /assets/cdn/leaders/$1 permanent;
        }
        location ~ ^/assets/ui/(.+)$ {
            rewrite ^/assets/ui/(.+)$ /assets/cdn/ui/$1 permanent;
        }

        location /assets/ {
            add_header Access-Control-Allow-Origin "*" always;
            add_header Access-Control-Allow-Methods "GET, OPTIONS" always;
            add_header Cache-Control "public, max-age=2592000";
            try_files $uri =404;
        }

        location = /health {
            default_type text/plain;
            try_files /health =404;
        }

        location = / {
            try_files /index.html =404;
        }

        location = /privacy {
            try_files /privacy/index.html =404;
        }

        location = /terms {
            try_files /terms/index.html =404;
        }

        location = /site.css {
            add_header Cache-Control "public, max-age=3600";
            try_files /site.css =404;
        }

        location ~ ^/relay/(?<relay_port>\d+)/ws/ {
            proxy_pass http://127.0.0.1:$relay_port/ws/;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "Upgrade";
            proxy_set_header Host $host;
            proxy_read_timeout 86400;
        }

        location ~ ^/lobby/(?<lobby_port>\d+)/ws/ {
            proxy_pass http://127.0.0.1:$lobby_port;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "Upgrade";
            proxy_set_header Host $host;
            proxy_read_timeout 86400;
        }

        location /ws/ {
            proxy_pass http://127.0.0.1:25565;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "Upgrade";
            proxy_set_header Host $host;
            proxy_read_timeout 86400;
        }

        location /maps/ {
            proxy_pass http://127.0.0.1:25566/maps/;
            proxy_set_header Host $host;
        }
      '';
    };

    virtualHosts."play.shadowsofwar.io" = {
      serverName = "play.shadowsofwar.io";
      root = "/var/www/play.shadowsofwar.io/html";
      enableACME = true;
      forceSSL = true;
      extraConfig = ''
        index index.html;

        gzip_vary on;
        gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript application/wasm image/svg+xml;

        location = /sow.svg {
            ${playCsp}
            add_header Cache-Control "public, max-age=2592000";
            try_files $uri =404;
        }

        location ~* ^/sow_client_[0-9]+(_bg)?\.(js|wasm)(\.br)?$ {
            ${playCsp}
            add_header Cache-Control "public, max-age=31536000, immutable";
            try_files $uri =404;
        }

        location = /game-manifest.json {
            ${playCsp}
            add_header Cache-Control "no-cache, must-revalidate";
            try_files $uri =404;
        }

        location = /sw.js {
            ${playCsp}
            add_header Cache-Control "no-cache, must-revalidate";
            try_files $uri =404;
        }

        location = /index.html {
            ${playCsp}
            add_header Cache-Control "no-cache, must-revalidate";
            try_files $uri =404;
        }

        location /assets/static/ {
            ${playCsp}
            add_header Cache-Control "public, max-age=2592000";
            try_files $uri =404;
        }

        location / {
            ${playCsp}
            try_files $uri $uri/ /index.html;
        }
      '';
    };

    virtualHosts."ptr.shadowsofwar.io" = {
      serverName = "ptr.shadowsofwar.io";
      root = "/var/www/ptr.shadowsofwar.io/html";
      enableACME = true;
      forceSSL = true;
      extraConfig = ''
        index index.html;

        gzip_vary on;
        gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript application/wasm image/svg+xml;

        location = /sow.svg {
            ${ptrRobots}
            add_header Cache-Control "public, max-age=2592000";
            try_files $uri =404;
        }

        location ~* ^/sow_client_[0-9]+(_bg)?\.(js|wasm)(\.br)?$ {
            ${ptrRobots}
            add_header Cache-Control "public, max-age=31536000, immutable";
            try_files $uri =404;
        }

        location = /game-manifest.json {
            ${ptrRobots}
            add_header Cache-Control "no-cache, must-revalidate";
            try_files $uri =404;
        }

        location = /sw.js {
            ${ptrRobots}
            add_header Cache-Control "no-cache, must-revalidate";
            try_files $uri =404;
        }

        location = /index.html {
            ${ptrRobots}
            add_header Cache-Control "no-cache, must-revalidate";
            try_files $uri =404;
        }

        location /assets/static/ {
            ${ptrRobots}
            add_header Cache-Control "public, max-age=2592000";
            try_files $uri =404;
        }

        location / {
            ${ptrRobots}
            try_files $uri $uri/ /index.html;
        }

        location /ws/ {
            ${ptrRobots}
            proxy_pass http://127.0.0.1:25575;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "Upgrade";
            proxy_set_header Host $host;
            proxy_read_timeout 86400;
        }

        location /maps/ {
            ${ptrRobots}
            proxy_pass http://127.0.0.1:25576/maps/;
            proxy_set_header Host $host;
        }
      '';
    };
  };
}
