"""Flow2API - Main Entry Point"""

from src.main import app
import uvicorn

if __name__ == "__main__":
    from src.core.config import config

    uvicorn.run(
        "src.main:app", host=config.server_host, port=config.server_port, reload=False
    )
